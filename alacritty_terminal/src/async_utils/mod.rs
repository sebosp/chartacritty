//! `async_utils` contains asynchronous functionality to avoid using the
//! terminal main thread to load data from remote sources or perform
//! calculations that would otherwise slow down the terminal behavior.
//! An async_coordinator is defined that receives requests over a futures mpsc
//! channel that may contain new data, may request OpenGL data or increment
//! internal counters.
use crate::charts::config::Config;
use crate::charts::{prometheus, ChartSizeInfo, ChartsConfig, TimeSeriesChart, TimeSeriesSource};
use crate::event::{Event, EventListener};
use crate::term::SizeInfo;
use log::*;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, interval_at};
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

/// `AsyncTask` contains message types that async_coordinator can work on
#[derive(Debug)]
pub enum AsyncTask {
    LoadResponse(MetricRequest),
    SendMetricsOpenGLData(usize, usize, oneshot::Sender<(Vec<f32>, f32)>),
    SendChartDecorationsOpenGLData(usize, usize, oneshot::Sender<(Vec<f32>, f32)>),
    ChangeDisplaySize(f32, f32, f32, f32, oneshot::Sender<bool>),
    IncrementInputCounter(u64, f64),
    IncrementOutputCounter(u64, f64),
    DecorUpdate(usize, f32),
    Shutdown,
    // Maybe add CloudWatch/etc
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

/// `load_http_response` handles the async_coordinator task of type LoadResponse
/// Currently only PrometheusTimeSeries are handled.
pub fn load_http_response(
    charts: &mut Vec<TimeSeriesChart>,
    response: MetricRequest,
    size: ChartSizeInfo,
) -> Option<usize> {
    // XXX: Move to prometheus.rs?
    let span = span!(Level::DEBUG, "load_http_response", idx = response.chart_index);
    let _enter = span.enter();
    if let Some(data) = response.data {
        if data.status != "success" {
            return None;
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
                        event!(
                            Level::DEBUG,
                            "load_http_response:(Chart: {}, Series: {}) {} records from {} into \
                             TimeSeries",
                            response.chart_index,
                            response.series_index,
                            num_records,
                            response.source_url
                        );
                        ok_records = num_records;
                    },
                    Err(err) => {
                        event!(
                            Level::DEBUG,
                            "load_http_response:(Chart: {}, Series: {}) Error Loading {} into \
                             TimeSeries: {:?}",
                            response.chart_index,
                            response.series_index,
                            response.source_url,
                            err
                        );
                    },
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
        let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        increment_internal_counter(charts, "async_loaded_items", now, ok_records as f64, size);
        Some(ok_records)
    } else {
        None
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
        },
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
/// SendChartDecorationsOpenGLData, it returns the chart index as opengl vertices
/// representation and the alpha through the channel parameter
pub fn send_chart_decorations_opengl_data(
    charts: &[TimeSeriesChart],
    chart_index: usize,
    data_index: usize,
    channel: oneshot::Sender<(Vec<f32>, f32)>,
) {
    event!(Level::DEBUG, "send_decorations_vecs for chart_index: {}", chart_index);
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
        },
        Err(err) => event!(Level::ERROR, "send_decorations_opengl_data: Error sending: {:?}", err),
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
        Err(err) => event!(Level::ERROR, "change_display_size: Error sending: {:?}", err),
    };
}

/// `async_coordinator` receives messages from the tasks about data loaded from
/// the network, it owns the charts array and is the single point by which data can
/// be loaded or requested. XXX: Config updates are not possible yet.
pub async fn async_coordinator<U>(
    mut rx: mpsc::Receiver<AsyncTask>,
    mut chart_config: crate::charts::ChartsConfig,
    size_info: SizeInfo,
    event_proxy: U,
) where
    U: EventListener + Send + 'static,
{
    event!(Level::DEBUG, "async_coordinator: Starting, terminal size info: {:?}", size_info,);
    // This Instant is synchronized with the decorations thread, mainly used so that decorations
    // are ran under specific circumstances
    chart_config.setup_chart_spacing();
    for chart in &mut chart_config.charts {
        // Calculate the spacing between charts
        event!(Level::DEBUG, "Finishing setup for sources in chart: '{}'", chart.name);
        for series in &mut chart.sources {
            series.init();
        }
    }
    let mut size = ChartSizeInfo { term_size: size_info, ..ChartSizeInfo::default() };
    while let Some(message) = rx.recv().await {
        event!(Level::DEBUG, "async_coordinator: message: {:?}", message);
        match message {
            AsyncTask::LoadResponse(req) => {
                if let Some(_items) = load_http_response(&mut chart_config.charts, req, size) {
                    chart_config.sync_latest_epoch(size);
                    event_proxy.send_event(Event::ChartEvent);
                }
            },
            AsyncTask::SendMetricsOpenGLData(chart_index, data_index, channel) => {
                send_metrics_opengl_vecs(&chart_config.charts, chart_index, data_index, channel);
            },
            AsyncTask::SendChartDecorationsOpenGLData(chart_index, data_index, channel) => {
                send_chart_decorations_opengl_data(
                    &chart_config.charts,
                    chart_index,
                    data_index,
                    channel,
                );
            },
            AsyncTask::ChangeDisplaySize(height, width, padding_y, padding_x, channel) => {
                change_display_size(
                    &mut chart_config.charts,
                    &mut size,
                    height,
                    width,
                    padding_y,
                    padding_x,
                    channel,
                );
            },
            AsyncTask::IncrementInputCounter(epoch, value) => {
                increment_internal_counter(&mut chart_config.charts, "input", epoch, value, size);
            },
            AsyncTask::IncrementOutputCounter(epoch, value) => {
                increment_internal_counter(&mut chart_config.charts, "output", epoch, value, size);
            },
            AsyncTask::DecorUpdate(_idx, _epoch_ms) => {
                event_proxy.send_event(Event::DecorEvent);
            },
            AsyncTask::Shutdown => {
                break;
            },
        };
    }
    event!(Level::INFO, "async_coordinator: Exiting");
}
/// `fetch_prometheus_response` gets data from prometheus and once data is ready
/// it sends the results to the coordinator.
async fn fetch_prometheus_response(
    item: MetricRequest,
    tx: mpsc::Sender<AsyncTask>,
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
        },
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
                .send(AsyncTask::LoadResponse(MetricRequest {
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
                    "fetch_prometheus_response:(Chart: {}, Series: {}) unable to send data back \
                     to coordinator; err={:?}",
                    chart_index,
                    series_index,
                    err
                )
            }
            Ok(())
        },
    }
}

/// `spawn_decoration_intervals` iterates over the charts and sources
pub fn spawn_decoration_intervals(
    charts_tx: mpsc::Sender<AsyncTask>,
    tokio_handle: tokio::runtime::Handle,
) {
    tokio_handle.spawn(async move {
        // 10 FPS for decorations
        let mut interval = time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            match charts_tx.send(AsyncTask::DecorUpdate(0usize, 0f32)).await {
                Ok(()) => {},
                Err(err) => error!("Unable to send DecorUpdate: {:?}", err),
            };
        }
    });
}

/// `spawn_charts_intervals` iterates over the charts and sources
/// and, if PrometheusTimeSeries it would call the spawn_datasource_interval_polls on it,
/// that would be constantly loading data asynchronously.
pub fn spawn_charts_intervals(
    charts: Vec<TimeSeriesChart>,
    charts_tx: mpsc::Sender<AsyncTask>,
    tokio_handle: tokio::runtime::Handle,
) {
    for (chart_index, chart) in charts.into_iter().enumerate() {
        for (series_index, series) in chart.sources.into_iter().enumerate() {
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
                    spawn_datasource_interval_polls(&data_request, charts_tx).await.unwrap_or_else(
                        |_| {
                            panic!(
                                "spawn_charts_intervals:(Chart: {}, Series: {}) Error spawning \
                                 datasource internal polls",
                                chart_index, series_index
                            )
                        },
                    );
                });
            }
        }
    }
}
/// `spawn_datasource_interval_polls` creates intervals for each series requested
/// Each series will have to reply to a mspc tx with the data
pub async fn spawn_datasource_interval_polls(
    item: &MetricRequest,
    tx: mpsc::Sender<AsyncTask>,
) -> Result<(), ()> {
    event!(
        Level::DEBUG,
        "spawn_datasource_interval_polls:(Chart: {}, Series: {}) Starting for item={:?}",
        item.chart_index,
        item.series_index,
        item
    );
    let mut interval =
        interval_at(tokio::time::Instant::now(), Duration::from_secs(item.pull_interval));
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
            },
            Err(()) => return Err(()),
        }
    }
    // How do we return Ok(())?
}

/// `get_metric_opengl_data` generates a oneshot::channel to communicate
/// with the async coordinator and request the vectors of the metric_data
/// or the decorations vertices, along with its alpha
pub fn get_metric_opengl_data(
    charts_tx: mpsc::Sender<AsyncTask>,
    chart_idx: usize,
    series_idx: usize,
    request_type: &'static str,
    tokio_handle: tokio::runtime::Handle,
) -> (Vec<f32>, f32) {
    let (opengl_tx, opengl_rx) = oneshot::channel();
    let chart_idx_bkp = chart_idx;
    tokio_handle.spawn(async move {
        let get_metric_request = charts_tx.send(if request_type == "metric_data" {
            AsyncTask::SendMetricsOpenGLData(chart_idx, series_idx, opengl_tx)
        } else {
            AsyncTask::SendChartDecorationsOpenGLData(chart_idx, series_idx, opengl_tx)
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
    // .expect(&format!(
    // "get_metric_opengl_data:(Chart: {}, Series: {}) Unable to spawn get_opengl_task",
    // chart_idx, series_idx
    // ));
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
            },
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
            },
        }
    })
}

/// `spawn_async_tasks` Starts a background thread to be used for tokio for async tasks
pub fn spawn_async_tasks<U>(
    chart_config: &ChartsConfig,
    charts_tx: mpsc::Sender<AsyncTask>,
    charts_rx: mpsc::Receiver<AsyncTask>,
    handle_tx: std::sync::mpsc::Sender<tokio::runtime::Handle>,
    size_info: SizeInfo,
    event_proxy: U,
) -> thread::JoinHandle<()>
where
    U: EventListener + Send + 'static,
{
    // let decor_config = config.decorations.clone();
    let chart_config = chart_config.clone();
    ::std::thread::Builder::new()
        .name("async I/O".to_owned())
        .spawn(move || {
            let tokio_runtime =
                tokio::runtime::Runtime::new().expect("Failed to start new tokio Runtime");
            info!("Tokio runtime created.");

            // Give a handle to the runtime back to the main thread.
            handle_tx
                .send(tokio_runtime.handle().clone())
                .expect("Unable to give runtime handle to the main thread");
            let chart_array = chart_config.charts.clone();
            let async_chart_config = chart_config.clone();
            let tokio_handle = tokio_runtime.handle().clone();
            let charts_tx_cp = charts_tx.clone();
            tokio_runtime.spawn(async {
                spawn_charts_intervals(chart_array, charts_tx_cp, tokio_handle);
            });
            let tokio_handle = tokio_runtime.handle().clone();
            tokio_runtime.spawn(async {
                spawn_decoration_intervals(charts_tx, tokio_handle);
            });
            tokio_runtime.block_on(async {
                async_coordinator(charts_rx, async_chart_config, size_info, event_proxy).await
            });
            info!("Tokio runtime finished.");
        })
        .expect("Unable to start async I/O thread")
}

/// `run` is an example use of the crate without drawing the data.
pub fn run<U>(config: Config, event_proxy: U)
where
    U: EventListener + Send + 'static,
{
    let chart_config = if let Some(chart_config) = config.charts {
        chart_config
    } else {
        return;
    };
    let size_info = SizeInfo {
        width: 100.,
        height: 100.,
        cell_width: 0.,
        cell_height: 0.,
        padding_x: 0.,
        padding_y: 0.,
        ..SizeInfo::default()
    };
    // Create the channel that is used to communicate with the
    // charts background task.
    let (charts_tx, charts_rx) = mpsc::channel(4_096usize);
    // Create a channel to receive a handle from Tokio
    //
    let (handle_tx, handle_rx) = std::sync::mpsc::channel();
    // Start the Async I/O runtime, this needs to run in a background thread because in OSX, only
    // the main thread can write to the graphics card.
    let tokio_thread = spawn_async_tasks(
        &chart_config,
        charts_tx.clone(),
        charts_rx,
        handle_tx,
        size_info,
        event_proxy,
    );
    let tokio_handle =
        handle_rx.recv().expect("Unable to get the tokio handle in a background thread");

    // Load some data, fetch the data and draw it.
    tokio_handle.spawn(async move { charts_tx.send(AsyncTask::Shutdown).await });

    // Terminate the background therad:
    tokio_thread.join().expect("Unable to shutdown tokio channel");
}
