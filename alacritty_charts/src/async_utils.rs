//! `async_utils` contains asynchronous functionality to avoid using the
//! terminal main thread to load data from remote sources or perform
//! calculations that would otherwise slow down the terminal behavior.
//! An async_coordinator is defined that receives requests over a futures mpsc
//! channel that may contain new data, may request OpenGL data or increment
//! internal counters.
use crate::prometheus;
use crate::ChartSizeInfo;
use crate::TimeSeriesChart;
use crate::TimeSeriesSource;
use alacritty_common::SizeInfo;
use futures::future::lazy;
use log::*;
use std::thread;
use std::time::Duration;
use std::time::UNIX_EPOCH;
use tokio::sync::{mpsc, oneshot};
use tokio::time::interval_at;
use tracing::{event, span, Level};

/// `MetricRequest` defines remote data sources that should be loaded regularly
#[derive(Debug, Clone)]
pub struct MetricRequest {
    pub pull_interval: u64,
    pub source_url: String,
    pub chart_index: usize,  // For Vec<TimeSeriesChart>
    pub series_index: usize, // For Vec<TimeSeriesSource>
    pub data: Option<prometheus::HTTPResponse>,
    pub capacity: usize, // This maps to the time range in seconds to query.
}

/// `AsyncChartTask` contains message types that async_coordinator can work on
#[derive(Debug)]
pub enum AsyncChartTask {
    LoadResponse(MetricRequest),
    SendMetricsOpenGLData(usize, usize, oneshot::Sender<(Vec<f32>, f32)>),
    SendDecorationsOpenGLData(usize, usize, oneshot::Sender<(Vec<f32>, f32)>),
    ChangeDisplaySize(f32, f32, f32, f32, oneshot::Sender<bool>),
    SendLastUpdatedEpoch(oneshot::Sender<u64>),
    IncrementInputCounter(u64, f64),
    IncrementOutputCounter(u64, f64),
    // Maybe add CloudWatch/etc
}

/// `get_last_updated_chart_epoch` Sends a request to the async_coordinator to
/// get the latest update epoch of all the charts. This is used so that we know
/// if the charts should be redrawn or not. XXX: This is not used yet.
pub async fn get_last_updated_chart_epoch(
    charts_tx: &mut mpsc::Sender<AsyncChartTask>,
    tokio_handle: tokio::runtime::Handle,
) -> u64 {
    let (chart_tx, chart_rx) = oneshot::channel();
    let get_latest_update_epoch = charts_tx
        .send(AsyncChartTask::SendLastUpdatedEpoch(chart_tx))
        .await
        .map_err(|e| error!("Sending SendLastUpdatedEpoch Task: err={:?}", e))
        .and_then(move |_res| {
            debug!("Sent Request for SendLastUpdatedEpoch");
            Ok(())
        });
    tokio_handle.spawn(lazy(move |_| get_latest_update_epoch));
    //.expect("Unable to queue async task for get_latest_update_epoch");
    let chart_rx = chart_rx.await;
    match chart_rx {
        Ok(data) => {
            debug!("Got response from SendLastUpdatedEpoch Task: {:?}", data);
            data
        }
        Err(err) => {
            error!("Error response from SendLastUpdatedEpoch Task: {:?}", err);
            0u64
        }
    }
}

/// `increment_internal_counter` handles a request to increment different
/// internal counter types.
pub fn increment_internal_counter(
    charts: &mut Vec<TimeSeriesChart>,
    counter_type: &'static str,
    epoch: u64,
    value: f64,
    size: ChartSizeInfo,
) {
    for chart in charts {
        let mut chart_updated = false;
        for series in &mut chart.sources {
            if counter_type == "input" {
                if let TimeSeriesSource::AlacrittyInput(ref mut input) = series {
                    input.series.upsert((epoch, Some(value)));
                    chart_updated = true;
                }
            }
            if counter_type == "output" {
                if let TimeSeriesSource::AlacrittyOutput(ref mut output) = series {
                    output.series.upsert((epoch, Some(value)));
                    chart_updated = true;
                }
            }
            // Update the loaded item counters
            if counter_type == "async_loaded_items" {
                if let TimeSeriesSource::AsyncLoadedItems(ref mut items) = series {
                    items.series.upsert((epoch, Some(value)));
                    chart_updated = true;
                }
            }
        }
        if chart_updated {
            chart.synchronize_series_epoch_range();
            chart.update_all_series_opengl_vecs(size);
        }
    }
}

/// `send_last_updated_epoch` handles the async_coordinator task of type
/// SendLastUpdatedEpoch. Once the max epoch of all the charts is known, it is
/// inserted it on the other series so that they also progress in time.
pub fn send_last_updated_epoch(charts: &mut Vec<TimeSeriesChart>, channel: oneshot::Sender<u64>) {
    // Under different timezones, this probably makes no sense
    let max: u64 = charts
        .iter()
        .map(|x| x.last_updated)
        .max()
        .unwrap_or_else(|| 0u64);
    let updated_charts: usize = charts
        .iter_mut()
        .map(|x| {
            if x.last_updated < max {
                x.sources
                    .iter_mut()
                    .map(|x| x.series_mut().upsert((max, None)))
                    .sum()
            } else {
                0usize
            }
        })
        .sum();
    debug!(
        "send_last_updated_epoch: Progressed {} series to {} epoch",
        updated_charts, max
    );
    match channel.send(max) {
        Ok(()) => {
            debug!(
                "send_last_updated_epoch: oneshot::message sent with payload {}",
                charts
                    .iter()
                    .map(|x| x.last_updated)
                    .max()
                    .unwrap_or_else(|| 0u64)
            );
        }
        Err(err) => error!("send_last_updated_epoch: Error sending: {:?}", err),
    };
}

/// `load_http_response` handles the async_coordinator task of type LoadResponse
/// Currently only PrometheusTimeSeries are handled.
pub fn load_http_response(
    charts: &mut Vec<TimeSeriesChart>,
    response: MetricRequest,
    size: ChartSizeInfo,
) {
    let span = span!(
        Level::DEBUG,
        "load_http_response",
        idx = response.chart_index
    );
    let _enter = span.enter();
    if let Some(data) = response.data {
        if data.status != "success" {
            return;
        }
        let mut ok_records = 0;
        if response.chart_index < charts.len()
            && response.series_index < charts[response.chart_index].sources.len()
        {
            if let TimeSeriesSource::PrometheusTimeSeries(ref mut prom) =
                charts[response.chart_index].sources[response.series_index]
            {
                match prom.load_prometheus_response(data) {
                    Ok(num_records) => {
                        event!(Level::INFO,
                            "load_http_response:(Chart: {}, Series: {}) {} records from {} into TimeSeries",
                            response.chart_index, response.series_index, num_records, response.source_url
                        );
                        ok_records = num_records;
                    }
                    Err(err) => {
                        event!(Level::DEBUG,
                            "load_http_response:(Chart: {}, Series: {}) Error Loading {} into TimeSeries: {:?}",
                            response.chart_index, response.series_index, response.source_url, err
                        );
                    }
                }
                event!(
                    Level::DEBUG,
                    "load_http_response:(Chart: {}, Series: {}) After loading. TimeSeries is: {:?}",
                    response.chart_index,
                    response.series_index,
                    charts[response.chart_index].sources[response.series_index]
                );
            }
            charts[response.chart_index].synchronize_series_epoch_range();
            charts[response.chart_index].update_all_series_opengl_vecs(size);
        }
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        increment_internal_counter(charts, "async_loaded_items", now, ok_records as f64, size);
    }
}

/// `send_metrics_opengl_vecs` handles the async_coordinator task of type
/// SendMetricsOpenGLData, it sends the logged metrics as vertices
/// representation through the channel parameter. The vertices are deduplicated
/// for troubleshooting purposes mostly.
pub fn send_metrics_opengl_vecs(
    charts: &[TimeSeriesChart],
    chart_index: usize,
    series_index: usize,
    channel: oneshot::Sender<(Vec<f32>, f32)>,
) {
    event!(
        Level::DEBUG,
        "send_metrics_opengl_vecs:(Chart: {}, Series: {}): Request received",
        chart_index,
        series_index
    );
    match channel.send(
        if chart_index >= charts.len() || series_index >= charts[chart_index].sources.len() {
            (vec![], 0.0f32)
        } else {
            (
                charts[chart_index].get_deduped_opengl_vecs(series_index),
                charts[chart_index].sources[series_index].alpha(),
            )
        },
    ) {
        Ok(()) => {
            event!(
                Level::DEBUG,
                "send_metrics_opengl_vecs:(Chart: {}, Series: {}) oneshot::message sent",
                chart_index,
                series_index
            );
        }
        Err(err) => event!(
            Level::ERROR,
            "send_metrics_opengl_vecs:(Chart: {}, Series: {}) Error sending: {:?}",
            chart_index,
            series_index,
            err
        ),
    };
}

/// `send_decorations_opengl_data` handles the async_coordinator task of type
/// SendDecorationsOpenGLData, it returns the chart index as opengl vertices
/// representation and the alpha through the channel parameter
pub fn send_decorations_opengl_data(
    charts: &[TimeSeriesChart],
    chart_index: usize,
    data_index: usize,
    channel: oneshot::Sender<(Vec<f32>, f32)>,
) {
    event!(
        Level::DEBUG,
        "send_decorations_vecs for chart_index: {}",
        chart_index
    );
    match channel.send(
        if chart_index >= charts.len() || data_index >= charts[chart_index].decorations.len() {
            (vec![], 0f32)
        } else {
            event!(
                Level::DEBUG,
                "send_decorations_opengl_data Sending vertices: {:?}",
                charts[chart_index].decorations[data_index].opengl_vertices()
            );
            (
                charts[chart_index].decorations[data_index].opengl_vertices(),
                charts[chart_index].decorations[data_index].alpha(),
            )
        },
    ) {
        Ok(()) => {
            event!(
                Level::DEBUG,
                "send_decorations_opengl_data: oneshot::message sent for index: {}",
                chart_index
            );
        }
        Err(err) => event!(
            Level::ERROR,
            "send_decorations_opengl_data: Error sending: {:?}",
            err
        ),
    };
}

/// `change_display_size` handles changes to the Display resizes.
/// It is debatable that we need to handle this message or return
/// anything, so we'll just return a true ACK, the charts are updated
/// after the size changes, potentially could be slow and we should delay
/// until the size is stabilized.
pub fn change_display_size(
    charts: &mut Vec<TimeSeriesChart>,
    size: &mut ChartSizeInfo,
    height: f32,
    width: f32,
    padding_y: f32,
    padding_x: f32,
    channel: oneshot::Sender<bool>,
) {
    event!(
        Level::DEBUG,
        "change_display_size for height: {}, width: {}, padding_y: {}, padding_x: {}",
        height,
        width,
        padding_y,
        padding_x
    );
    size.term_size.height = height;
    size.term_size.width = width;
    size.term_size.padding_y = padding_y;
    size.term_size.padding_x = padding_x;
    for chart in charts {
        // Update the OpenGL representation when the display changes
        chart.update_all_series_opengl_vecs(*size);
    }
    match channel.send(true) {
        Ok(()) => event!(
            Level::DEBUG,
            "change_display_size: Sent reply back to resize notifier, new size: {:?}",
            size
        ),
        Err(err) => event!(
            Level::ERROR,
            "change_display_size: Error sending: {:?}",
            err
        ),
    };
}

/// `async_coordinator` receives messages from the tasks about data loaded from
/// the network, it owns the charts array and is the single point by which data can
/// be loaded or requested. XXX: Config updates are not possible yet.
pub async fn async_coordinator(
    mut rx: mpsc::Receiver<AsyncChartTask>,
    mut chart_config: crate::ChartsConfig,
    height: f32,
    width: f32,
    padding_y: f32,
    padding_x: f32,
) {
    event!(
        Level::DEBUG,
        "async_coordinator: Starting, terminal height: {}, width: {}, padding_y: {}, padding_x {}",
        height,
        width,
        padding_y,
        padding_x
    );
    chart_config.setup_chart_spacing();
    for chart in &mut chart_config.charts {
        // Caculate the spacing between charts
        event!(
            Level::DEBUG,
            "Finishing setup for sources in chart: '{}'",
            chart.name
        );
        for series in &mut chart.sources {
            series.init();
        }
    }
    let mut size = ChartSizeInfo {
        term_size: SizeInfo {
            height,
            width,
            padding_y,
            padding_x,
            ..SizeInfo::default()
        },
        ..ChartSizeInfo::default()
    };
    while let Some(message) = rx.recv().await {
        event!(Level::DEBUG, "async_coordinator: message: {:?}", message);
        match message {
            AsyncChartTask::LoadResponse(req) => {
                load_http_response(&mut chart_config.charts, req, size)
            }
            AsyncChartTask::SendMetricsOpenGLData(chart_index, data_index, channel) => {
                send_metrics_opengl_vecs(&chart_config.charts, chart_index, data_index, channel);
            }
            AsyncChartTask::SendDecorationsOpenGLData(chart_index, data_index, channel) => {
                send_decorations_opengl_data(
                    &chart_config.charts,
                    chart_index,
                    data_index,
                    channel,
                );
            }
            AsyncChartTask::ChangeDisplaySize(height, width, padding_y, padding_x, channel) => {
                change_display_size(
                    &mut chart_config.charts,
                    &mut size,
                    height,
                    width,
                    padding_y,
                    padding_x,
                    channel,
                );
            }
            AsyncChartTask::IncrementInputCounter(epoch, value) => {
                increment_internal_counter(&mut chart_config.charts, "input", epoch, value, size);
            }
            AsyncChartTask::IncrementOutputCounter(epoch, value) => {
                increment_internal_counter(&mut chart_config.charts, "output", epoch, value, size);
            }
            AsyncChartTask::SendLastUpdatedEpoch(channel) => {
                send_last_updated_epoch(&mut chart_config.charts, channel);
            }
        };
    }
    event!(
        Level::ERROR,
        "async_coordinator: Exiting. This shouldn't happen"
    );
}
/// `fetch_prometheus_response` gets data from prometheus and once data is ready
/// it sends the results to the coordinator.
async fn fetch_prometheus_response(
    item: MetricRequest,
    mut tx: mpsc::Sender<AsyncChartTask>,
) -> Result<(), ()> {
    event!(
        Level::DEBUG,
        "fetch_prometheus_response:(Chart: {}, Series: {}) Starting",
        item.chart_index,
        item.series_index
    );
    let url = prometheus::PrometheusTimeSeries::prepare_url(&item.source_url, item.capacity as u64)
        .unwrap();
    let url_copy = item.source_url.clone();
    let chart_index = item.chart_index;
    let series_index = item.series_index;
    let prom_res =
        prometheus::get_from_prometheus(url.clone(), Some(Duration::from_secs(item.pull_interval)))
            .await;
    match prom_res {
        Err(e) => {
            // e contains (Uri, Err)
            let (uri, error) = e;
            if error.is_timeout() {
                event!(
                    Level::INFO,
                    "fetch_prometheus_response:(Chart: {}, Series: {}) TimeOut accesing: {}",
                    chart_index,
                    series_index,
                    url_copy
                );
            } else {
                event!(
                    Level::INFO,
                    "fetch_prometheus_response:(Chart: {}, Series: {}) url={:?}, err={:?}",
                    chart_index,
                    series_index,
                    uri,
                    error
                );
            };
            // Instead of an error, return this so we can retry later.
            // XXX: Maybe exponential retries in the future.
            Ok(())
        }
        Ok(value) => {
            event!(
                Level::DEBUG,
                "fetch_prometheus_response:(Chart: {}, Series: {}) Prometheus raw value={:?}",
                chart_index,
                series_index,
                value
            );
            let res = prometheus::parse_json(&item.source_url, &value);
            let tx_res = tx
                .send(AsyncChartTask::LoadResponse(MetricRequest {
                    source_url: item.source_url.clone(),
                    chart_index: item.chart_index,
                    series_index: item.series_index,
                    pull_interval: item.pull_interval,
                    data: res.clone(),
                    capacity: item.capacity,
                }))
                .await;
            if let Err(err) = tx_res {
                event!(
                Level::ERROR,
                    "fetch_prometheus_response:(Chart: {}, Series: {}) unable to send data back to coordinator; err={:?}",
                    chart_index, series_index, err
                )
            }
            Ok(())
        }
    }
}

/// `spawn_charts_intervals` iterates over the charts and sources
/// and, if PrometheusTimeSeries it would call the spawn_datasource_interval_polls on it,
/// that would be constantly loading data asynchronously.
pub fn spawn_charts_intervals(
    charts: Vec<TimeSeriesChart>,
    charts_tx: mpsc::Sender<AsyncChartTask>,
    tokio_handle: tokio::runtime::Handle,
) {
    let mut chart_index = 0usize;
    for chart in charts {
        let mut series_index = 0usize;
        for series in chart.sources {
            if let TimeSeriesSource::PrometheusTimeSeries(ref prom) = series {
                event!(
                    Level::DEBUG,
                    "spawn_charts_intervals:(Chart: {}, Series: {}) - Adding interval run for '{}'",
                    chart_index,
                    series_index,
                    chart.name
                );
                let data_request = MetricRequest {
                    source_url: prom.source.clone(),
                    pull_interval: prom.pull_interval as u64,
                    chart_index,
                    series_index,
                    capacity: prom.series.metrics_capacity,
                    data: None,
                };
                let charts_tx = charts_tx.clone();
                tokio_handle.spawn(async move {
                    spawn_datasource_interval_polls(&data_request, charts_tx).await.expect(&format!("spawn_charts_intervals:(Chart: {}, Series: {}) Error spawning datasource internal polls", chart_index, series_index));
                });
            }
            series_index += 1;
        }
        chart_index += 1;
    }
}
/// `spawn_datasource_interval_polls` creates intervals for each series requested
/// Each series will have to reply to a mspc tx with the data
pub async fn spawn_datasource_interval_polls(
    item: &MetricRequest,
    tx: mpsc::Sender<AsyncChartTask>,
) -> Result<(), ()> {
    event!(
        Level::DEBUG,
        "spawn_datasource_interval_polls:(Chart: {}, Series: {}) Starting for item={:?}",
        item.chart_index,
        item.series_index,
        item
    );
    let mut interval = interval_at(
        tokio::time::Instant::now(),
        Duration::from_secs(item.pull_interval),
    );
    loop {
        interval.tick().await;
        let async_metric_item = MetricRequest {
            source_url: item.source_url.clone(),
            chart_index: item.chart_index,
            series_index: item.series_index,
            pull_interval: item.pull_interval,
            data: None,
            capacity: item.capacity,
        };
        event!(
            Level::DEBUG,
            "spawn_datasource_interval_polls:(Chart: {}, Series: {}) Interval triggered for {:?}",
            async_metric_item.chart_index,
            async_metric_item.series_index,
            async_metric_item.source_url
        );
        match fetch_prometheus_response(async_metric_item.clone(), tx.clone()).await {
            Ok(res) => {
                event!(
                    Level::DEBUG,
                    "spawn_datasource_interval_polls:(Chart: {}, Series: {}) Response {:?}",
                    async_metric_item.chart_index,
                    async_metric_item.series_index,
                    res
                );
            }
            Err(()) => return Err(()),
        }
    }
    // How do we return Ok(())?
}

/// `get_metric_opengl_data` generates a oneshot::channel to communicate
/// with the async coordinator and request the vectors of the metric_data
/// or the decorations vertices, along with its alpha
pub fn get_metric_opengl_data(
    mut charts_tx: mpsc::Sender<AsyncChartTask>,
    chart_idx: usize,
    series_idx: usize,
    request_type: &'static str,
    tokio_handle: tokio::runtime::Handle,
) -> (Vec<f32>, f32) {
    let (opengl_tx, opengl_rx) = oneshot::channel();
    let chart_idx_bkp = chart_idx.clone();
    tokio_handle.spawn(async move {
        let get_metric_request = charts_tx.send(if request_type == "metric_data" {
            AsyncChartTask::SendMetricsOpenGLData(chart_idx, series_idx, opengl_tx)
        } else {
            AsyncChartTask::SendDecorationsOpenGLData(chart_idx, series_idx, opengl_tx)
        });
        match get_metric_request.await {
            Err(e) => event!(
                Level::ERROR,
                "get_metric_opengl_data:(Chart: {}, Series: {}) Sending {} Task. err={:?}",
                chart_idx,
                series_idx,
                request_type,
                e
            ),
            Ok(_) => event!(
                Level::DEBUG,
                "get_metric_opengl_data:(Chart: {}, Series: {}) Sent Request for {} Task",
                chart_idx,
                series_idx,
                request_type
            ),
        }
    });
    /*.expect(&format!(
        "get_metric_opengl_data:(Chart: {}, Series: {}) Unable to spawn get_opengl_task",
        chart_idx, series_idx
    ));*/
    tokio_handle.block_on(async {
        match opengl_rx.await {
            Ok(data) => {
                event!(
                    Level::DEBUG,
                    "get_metric_opengl_data:(Chart: {}, Series: {}) Response from {} Task: {:?}",
                    chart_idx_bkp,
                    series_idx,
                    request_type,
                    data
                );
                data
            }
            Err(err) => {
                event!(
                    Level::ERROR,
                    "get_metric_opengl_data:(Chart: {}, Series: {}) Error from {} Task: {:?}",
                    chart_idx_bkp,
                    series_idx,
                    request_type,
                    err
                );
                (vec![], 0f32)
            }
        }
    })
}

/// `tokio_default_setup` creates a default channels and handles, this should be used mostly for testing
/// to avoid having to create all the tokio boilerplate, I would like to return a struct but
/// the ownership and cloning and moving of the separate parts does not seem possible then
pub fn tokio_default_setup() -> (
    tokio::runtime::Handle,
    mpsc::Sender<AsyncChartTask>,
    oneshot::Sender<()>,
) {
    // Create the channel that is used to communicate with the
    // charts background task.
    let (charts_tx, charts_rx) = mpsc::channel(4_096usize);
    // Create a channel to receive a handle from Tokio
    //
    let (handle_tx, handle_rx) = std::sync::mpsc::channel();
    // Start the Async I/O runtime, this needs to run in a background thread because in OSX,
    // only the main thread can write to the graphics card.
    let (_tokio_thread, tokio_shutdown) = spawn_async_tasks(
        Some(crate::ChartsConfig::default()),
        charts_tx.clone(),
        charts_rx,
        handle_tx,
        ChartSizeInfo::default(),
    );
    let tokio_handle = handle_rx
        .recv()
        .expect("Unable to get the tokio handle in a background thread");

    (tokio_handle, charts_tx, tokio_shutdown)
}

/// `spawn_async_tasks` Starts a background thread to be used for tokio for async tasks
pub fn spawn_async_tasks(
    chart_config: Option<crate::ChartsConfig>,
    charts_tx: mpsc::Sender<AsyncChartTask>,
    charts_rx: mpsc::Receiver<AsyncChartTask>,
    handle_tx: std::sync::mpsc::Sender<tokio::runtime::Handle>,
    charts_size_info: ChartSizeInfo,
) -> (thread::JoinHandle<()>, oneshot::Sender<()>) {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let tokio_thread = ::std::thread::Builder::new()
        .name("async I/O".to_owned())
        .spawn(move || {
            let mut tokio_runtime =
                tokio::runtime::Runtime::new().expect("Failed to start new tokio Runtime");
            info!("Tokio runtime created.");

            // Give a handle to the runtime back to the main thread.
            handle_tx
                .send(tokio_runtime.handle().clone())
                .expect("Unable to give runtime handle to the main thread");
            let mut chart_array: Vec<TimeSeriesChart> = vec![];
            if let Some(chart_config) = &chart_config {
                chart_array = chart_config.charts.clone();
                let async_chart_config = chart_config.clone();
                tokio_runtime.spawn(async move {
                    async_coordinator(
                        charts_rx,
                        async_chart_config,
                        charts_size_info.term_size.height,
                        charts_size_info.term_size.width,
                        charts_size_info.term_size.padding_y,
                        charts_size_info.term_size.padding_x,
                    )
                    .await;
                });
            }
            let chart_array = chart_array.clone();
            let tokio_handle = tokio_runtime.handle().clone();
            tokio_runtime.spawn(async {
                spawn_charts_intervals(chart_array, charts_tx, tokio_handle);
            });
            tokio_runtime.block_on(async {
                match shutdown_rx.await {
                    Ok(_) => info!("Got shutdown signal for Tokio"),
                    Err(err) => error!("Error on the tokio shutdown channel: {:?}", err),
                }
            });
            info!("Tokio runtime finished.");
        })
        .expect("Unable to start async I/O thread");
    (tokio_thread, shutdown_tx)
}

/// `run` is an example use of the crate without drawing the data.
pub fn run(config: crate::config::Config) {
    let charts_size_info = ChartSizeInfo {
        term_size: SizeInfo {
            width: 100.,
            height: 100.,
            cell_width: 0.,
            cell_height: 0.,
            padding_x: 0.,
            padding_y: 0.,
            ..SizeInfo::default()
        },
        chart_width: 100.,
        chart_height: 100.,
    };
    // Create the channel that is used to communicate with the
    // charts background task.
    let (charts_tx, charts_rx) = mpsc::channel(4_096usize);
    // Create a channel to receive a handle from Tokio
    //
    let (handle_tx, handle_rx) = std::sync::mpsc::channel();
    // Start the Async I/O runtime, this needs to run in a background thread because in OSX, only
    // the main thread can write to the graphics card.
    let (tokio_thread, tokio_shutdown) = spawn_async_tasks(
        config.charts.clone(),
        charts_tx.clone(),
        charts_rx,
        handle_tx,
        charts_size_info,
    );
    let _tokio_handle = handle_rx
        .recv()
        .expect("Unable to get the tokio handle in a background thread");

    // Load some data, fetch the data and draw it.

    // Terminate the background therad:
    tokio_shutdown
        .send(())
        .expect("Unable to send shutdown signal to tokio runtime");
    tokio_thread
        .join()
        .expect("Unable to shutdown tokio channel");
}
