//! `Prometheus HTTP API` data structures
use crate::charts::TimeSeries;
use crate::charts::ValueCollisionPolicy;
use crate::term::color::Rgb;
use hyper::client::connect::HttpConnector;
use hyper::Client;
use hyper_tls::HttpsConnector;
use log::*;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};
// The below data structures for parsing something like:
//  {
//   "data": {
//     "result": [
//       {
//         "metric": {
//           "__name__": "up",
//           "instance": "localhost:9090",
//           "job": "prometheus"
//         },
//         "value": [
//           1557052757.816,
//           "1"
//         ]
//       },{...}
//     ],
//     "resultType": "vector"
//   },
//   "status": "success"
// }
/// `HTTPMatrixResult` contains Range Vectors, data is stored like this
/// [[Epoch1, Metric1], [Epoch2, Metric2], ...]
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct HTTPMatrixResult {
    #[serde(rename = "metric")]
    pub labels: HashMap<String, String>,
    pub values: Vec<Vec<serde_json::Value>>,
}

/// `HTTPVectorResult` contains Instant Vectors, data is stored like this
/// [Epoch1, Metric1, Epoch2, Metric2, ...]
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct HTTPVectorResult {
    #[serde(rename = "metric")]
    pub labels: HashMap<String, String>,
    pub value: Vec<serde_json::Value>,
}

/// `HTTPResponseData` may be one of these types:
/// https://prometheus.io/docs/prometheus/latest/querying/api/#expression-query-result-formats
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag = "resultType")]
pub enum HTTPResponseData {
    #[serde(rename = "vector")]
    Vector { result: Vec<HTTPVectorResult> },
    #[serde(rename = "matrix")]
    Matrix { result: Vec<HTTPMatrixResult> },
    #[serde(rename = "scalar")]
    Scalar { result: Vec<serde_json::Value> },
    #[serde(rename = "string")]
    String { result: Vec<serde_json::Value> },
}

impl Default for HTTPResponseData {
    fn default() -> HTTPResponseData {
        HTTPResponseData::Vector { result: vec![HTTPVectorResult::default()] }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct HTTPResponse {
    pub data: HTTPResponseData,
    pub status: String,
}

/// Transforms an serde_json::Value into an optional u64
/// The epoch coming from is a float (epoch with millisecond),
/// but our internal representation is u64
pub fn prometheus_epoch_to_u64(input: &serde_json::Value) -> Option<u64> {
    if input.is_number() {
        let input = input.as_f64()?;
        return Some(input as u64);
    }
    None
}

/// Transforms an serde_json::Value into an optional f64
pub fn serde_json_to_num(input: &serde_json::Value) -> Option<f64> {
    if input.is_string() {
        let input = input.as_str()?;
        if let Ok(value) = input.parse() {
            return Some(value);
        }
    }
    None
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrometheusTimeSeries {
    /// The Name of this TimesSeries
    #[serde(default)]
    pub name: String,

    /// The TimeSeries metrics storage
    #[serde(default)]
    pub series: TimeSeries,

    /// The TimeSeries metrics storage
    #[serde(default)]
    pub data: HTTPResponseData,

    /// The URL were Prometheus metrics may be acquaired
    #[serde(default)]
    pub source: String,

    /// The URL were Prometheus metrics may be acquaired
    #[serde(skip)]
    pub url: hyper::Uri,

    /// A response may be vector, matrix, scalar or string
    #[serde(default)]
    pub data_type: String,

    /// The Labels key and value, if any, to match the response
    #[serde(default)]
    #[serde(rename = "labels")]
    pub required_labels: HashMap<String, String>,

    /// The time in secondso to get the metrics from Prometheus
    /// Shouldn't be faster than the scrape interval for the Target
    #[serde(default)]
    #[serde(rename = "refresh")]
    pub pull_interval: usize,

    /// The color of the TimeSeries
    #[serde(default)]
    pub color: Rgb,

    /// The transparency of the TimeSeries
    #[serde(default)]
    pub alpha: f32,
}

impl Default for PrometheusTimeSeries {
    fn default() -> PrometheusTimeSeries {
        PrometheusTimeSeries {
            name: String::from("Unset"),
            series: TimeSeries {
                collision_policy: ValueCollisionPolicy::Overwrite,
                ..TimeSeries::default()
            },
            data: HTTPResponseData::default(),
            source: String::from(""),
            url: hyper::Uri::default(),
            pull_interval: 15,
            data_type: String::from("vector"),
            required_labels: HashMap::new(),
            color: Rgb::default(),
            alpha: 1.0,
        }
    }
}
impl PrometheusTimeSeries {
    /// `new` returns a new PrometheusTimeSeries. it takes a URL where to load
    /// the data from and a pull_interval, this should match scrape interval in
    /// Prometheus Server side to avoid pulling the same values over and over.
    pub fn new(
        url_param: String,
        pull_interval: usize,
        data_type: String,
        required_labels: HashMap<String, String>,
    ) -> Result<PrometheusTimeSeries, String> {
        let mut res = PrometheusTimeSeries {
            name: String::from("Unset"),
            series: TimeSeries {
                collision_policy: ValueCollisionPolicy::Overwrite,
                ..TimeSeries::default()
            },
            data: HTTPResponseData::default(),
            source: url_param,
            url: hyper::Uri::default(),
            pull_interval,
            data_type,
            required_labels,
            ..PrometheusTimeSeries::default()
        };
        match PrometheusTimeSeries::prepare_url(&res.source, res.series.metrics_capacity as u64) {
            Ok(url) => {
                res.url = url;
                Ok(res)
            },
            Err(err) => Err(err),
        }
    }

    /// `init` sets up several properties that would be too complicated to setup via yaml config
    pub fn init(&mut self) {
        self.series.collision_policy = ValueCollisionPolicy::Overwrite;
    }

    /// `prepare_url` loads self.source into a hyper::Uri
    /// It also adds a epoch-start and epoch-end to the
    /// URL depending on the metrics capacity
    pub fn prepare_url(source: &str, metrics_capacity: u64) -> Result<hyper::Uri, String> {
        // url should be like ("http://localhost:9090/api/v1/query?{}",query)
        // We split self.source into url_base_path?params
        // XXX: We only support one param, if more params are added with &
        //      they are percent encoded.
        // But sounds like configuration would become easy to mess up.
        let url_parts: Vec<&str> = source.split('?').collect();
        if url_parts.len() < 2 {
            return Err(String::from(
                "Unable to get url_parts, expected http://host:port/location?params",
            ));
        }
        let url_base_path = url_parts[0];
        // XXX: We only support one input param
        let url_param = url_parts[1..].join("");
        let encoded_url_param = utf8_percent_encode(&url_param, DEFAULT_ENCODE_SET).to_string();
        let mut encoded_url = format!("{}?{}", url_base_path, encoded_url_param);
        // If this is a query_range, we need to add time range
        if encoded_url.contains("/api/v1/query_range?") {
            let end = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let start = end - metrics_capacity;
            let step = "1"; // Maybe we can change granularity later
            encoded_url = format!("{}&start={}&end={}&step={}", encoded_url, start, end, step);
        }
        match encoded_url.parse::<hyper::Uri>() {
            Ok(url) => {
                if url.scheme() == Some(&hyper::http::uri::Scheme::HTTP)
                    || url.scheme() == Some(&hyper::http::uri::Scheme::HTTPS)
                {
                    debug!("Setting url to: {:?}", url);
                    Ok(url)
                } else {
                    error!("Only HTTP and HTTPS protocols are supported");
                    Err(format!("Unsupported protocol: {:?}", url.scheme()))
                }
            },
            Err(err) => {
                error!("Unable to parse url: {}", err);
                Err(format!("Unable to parse URL: {:?}", err))
            },
        }
    }

    /// `match_metric_labels` checks the labels in the incoming
    /// PrometheusData contains the required labels
    pub fn match_metric_labels(&self, metric_labels: &HashMap<String, String>) -> bool {
        for (required_label, required_value) in &self.required_labels {
            match metric_labels.get(required_label) {
                Some(return_value) => {
                    if return_value == required_value {
                        debug!(
                            "Good: Required label '{}' exists and matches required value",
                            required_label
                        );
                    } else {
                        debug!(
                            "Skip: Required label '{}' exists but required value: '{}' does not \
                             match current value: '{}'",
                            required_label, required_value, return_value
                        );
                        return false;
                    }
                },
                None => {
                    debug!("Skip: Required label '{}' does not exists", required_label);
                    return false;
                },
            }
        }
        true
    }

    /// `load_prometheus_response` loads data from PrometheusResponse into
    /// the internal `series`, returns the number of items or an error
    /// string
    pub fn load_prometheus_response(&mut self, res: HTTPResponse) -> Result<usize, String> {
        let mut loaded_items = 0;
        if res.status != "success" {
            return Ok(0usize);
        }
        debug!("load_prometheus_response: before upsert, series is: {:?}", self.series);
        debug!("load_prometheus_response: Checking data: {:?}", res.data);
        match res.data {
            HTTPResponseData::Vector { result: results } => {
                // labeled metrics returned as a 2 items vector:
                // [ {metric: {l: X}, value: [epoch1,sample1]}
                //   {metric: {l: Y}, value: [epoch2,sample2]} ]
                for metric_data in results.iter() {
                    if self.match_metric_labels(&metric_data.labels) {
                        // The result array is  [epoch, value, epoch, value]
                        if metric_data.value.len() == 2 {
                            let opt_epoch = prometheus_epoch_to_u64(&metric_data.value[0]);
                            let value = serde_json_to_num(&metric_data.value[1]);
                            if let Some(epoch) = opt_epoch {
                                loaded_items += self.series.upsert((epoch, value));
                            }
                        }
                    }
                }
            },
            HTTPResponseData::Matrix { result: results } => {
                // labeled metrics returned as a matrix:
                // [ {metric: {l: X}, value: [[epoch1,sample2],[...]]}
                //   {metric: {l: Y}, value: [[epoch3,sample4],[...]]} ]
                for metric_data in results.iter() {
                    if self.match_metric_labels(&metric_data.labels) {
                        // The result array is  [epoch, value, epoch, value]
                        for item_value in &metric_data.values {
                            for item in item_value.chunks_exact(2) {
                                let opt_epoch = prometheus_epoch_to_u64(&item[0]);
                                let value = serde_json_to_num(&item[1]);
                                if let Some(epoch) = opt_epoch {
                                    loaded_items += self.series.upsert((epoch, value));
                                }
                            }
                        }
                    }
                }
            },
            HTTPResponseData::Scalar { result } | HTTPResponseData::String { result } => {
                // unlabeled metrics returned as a 2 items vector
                // [epoch1,sample2]
                // XXX: no example found for String.
                if result.len() > 1 {
                    let opt_epoch = prometheus_epoch_to_u64(&result[0]);
                    let value = serde_json_to_num(&result[1]);
                    if let Some(epoch) = opt_epoch {
                        loaded_items += self.series.upsert((epoch, value));
                    }
                }
            },
        };
        if loaded_items > 0 {
            self.series.calculate_stats();
        }
        debug!("load_prometheus_response: after upsert, series is: {:?}", self.series);
        Ok(loaded_items)
    }
}

/// `get_from_prometheus` is an async operation that returns an Optional
/// PrometheusResponse
pub async fn get_from_prometheus(
    url: hyper::Uri,
    connect_timeout: Option<Duration>,
) -> Result<hyper::body::Bytes, (hyper::Uri, hyper::error::Error)> {
    info!("get_from_prometheus: Loading Prometheus URL: {}", url);
    let request = if url.scheme() == Some(&hyper::http::uri::Scheme::HTTP) {
        Client::builder()
            .pool_idle_timeout(connect_timeout) // Is this the same as connect_timeout in Client?
            .build::<_, hyper::Body>(HttpConnector::new())
            .get(url.clone())
    } else {
        let https = HttpsConnector::new();
        Client::builder().build::<_, hyper::Body>(https).get(url.clone())
    };
    let url_copy = url.clone();
    match request.await {
        // Since we don't know the end yet, we can't simply stream
        // the chunks as they arrive as we did with the above uppercase endpoint.
        // So here we do `.await` on the future, waiting on concatenating the full body,
        Ok(res) => match hyper::body::to_bytes(res.into_body()).await {
            Ok(body) => Ok(body),
            Err(err) => Err((url_copy, err)),
        },
        Err(err) => {
            info!("get_from_prometheus: Error loading '{:?}': '{:?}'", url_copy, err);
            Err((url_copy, err))
        },
    }
}
/// `parse_json` transforms a hyper body chunk into a possible
/// PrometheusResponse, mostly used for testing
pub fn parse_json(url: &str, body: &hyper::body::Bytes) -> Option<HTTPResponse> {
    let prom_res: Result<HTTPResponse, serde_json::Error> = serde_json::from_slice(body);
    match prom_res {
        Ok(v) => {
            debug!("parse_json for '{}': returned JSON={:?}", url, v);
            Some(v)
        },
        Err(err) => {
            error!("parse_json for '{}': err={:?}. Input: {:?}", url, err, body);
            None
        },
    }
}
/// XXX: REMOVE
/// Implement PartialEq for PrometheusTimeSeries because the field
/// tokio_core should be ignored
impl PartialEq<PrometheusTimeSeries> for PrometheusTimeSeries {
    fn eq(&self, other: &PrometheusTimeSeries) -> bool {
        self.series == other.series
            && self.url == other.url
            && self.pull_interval == other.pull_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::prometheus::HTTPResponseData::Vector;
    use crate::charts::MissingValuesPolicy;
    use crate::charts::TimeSeries;
    use crate::charts::TimeSeriesStats;
    use crate::charts::UpsertType;
    fn init_log() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn it_skips_prometheus_errors() {
        // This URL has the end time BEFORE the start time
        init_log();
        let test0_res: Result<PrometheusTimeSeries, String> = PrometheusTimeSeries::new(
            String::from("http://localhost:9090/api/v1/query_range?query=node_load1&start=1558253499&end=1558253479&step=1"),
            15,
            String::from("matrix"),
            HashMap::new(),
        );
        assert!(test0_res.is_ok());
        // A json returned by prometheus
        let test0_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "error",
              "errorType": "bad_data",
              "error": "end timestamp must not be before start time"
            }
            "#,
        );
        let res0_json = parse_json(&String::from("http://test"), &test0_json);
        assert!(res0_json.is_none());
        let test1_json = hyper::body::Bytes::from("Internal Server Error");
        let res1_json = parse_json(&String::from("http://test"), &test1_json);
        assert!(res1_json.is_none());
    }

    #[test]
    fn it_loads_prometheus_scalars() {
        let test0_res: Result<PrometheusTimeSeries, String> = PrometheusTimeSeries::new(
            String::from("http://localhost:9090/api/v1/query?query=1"),
            15,
            String::from("scalar"),
            HashMap::new(),
        );
        assert!(test0_res.is_ok());
        let mut test0 = test0_res.unwrap();
        // A json returned by prometheus
        let test0_json = hyper::body::Bytes::from(
            r#"
            { "status":"success",
              "data":{
                "resultType":"scalar",
                "result":[1558283674.829,"1"]
              }
            }"#,
        );
        let res0_json = parse_json(&String::from("http://test"), &test0_json);
        assert!(res0_json.is_some());
        let res0_load = test0.load_prometheus_response(res0_json.unwrap());
        // 1 items should have been loaded
        assert_eq!(res0_load, Ok(1usize));
        // This json is missing the value after the epoch
        let test1_json = hyper::body::Bytes::from(
            r#"
            { "status":"success",
              "data":{
                "resultType":"scalar",
                "result":[1558283674.829]
              }
            }"#,
        );
        let res1_json = parse_json(&String::from("http://test"), &test1_json);
        assert!(res1_json.is_some());
        let res1_load = test0.load_prometheus_response(res1_json.unwrap());
        // 0 items should have been loaded, because there's no value
        assert_eq!(res1_load, Ok(0usize));
    }

    #[test]
    fn it_loads_prometheus_matrix() {
        init_log();
        let test0_res: Result<PrometheusTimeSeries, String> = PrometheusTimeSeries::new(
            String::from("http://localhost:9090/api/v1/query_range?query=node_load1&start=1558253469&end=1558253479&step=1"),
            15,
            String::from("matrix"),
            HashMap::new()
        );
        assert!(test0_res.is_ok());
        let mut test0 = test0_res.unwrap();
        // Let's create space for 15, but we will receive 11 records:
        test0.series = test0.series.with_capacity(15usize);
        // A json returned by prometheus
        let test0_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "matrix",
                "result": [
                  {
                    "metric": {
                      "__name__": "node_load1",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "values": [
                        [1558253469,"1.69"],[1558253470,"1.70"],[1558253471,"1.71"],
                        [1558253472,"1.72"],[1558253473,"1.73"],[1558253474,"1.74"],
                        [1558253475,"1.75"],[1558253476,"1.76"],[1558253477,"1.77"],
                        [1558253478,"1.78"],[1558253479,"1.79"]]
                  }
                ]
              }
            }"#,
        );
        let res0_json = parse_json(&String::from("http://test"), &test0_json);
        assert!(res0_json.is_some());
        let res0_load = test0.load_prometheus_response(res0_json.unwrap());
        // 11 items should have been loaded in the node_exporter
        assert_eq!(res0_load, Ok(11usize));
        debug!("it_loads_prometheus_matrix NOTVEC: {:?}", test0.series.metrics);
        let loaded_data = test0.series.as_vec();
        debug!("it_loads_prometheus_matrix Data: {:?}", loaded_data);
        assert_eq!(loaded_data[0], (1558253469, Some(1.69f64)));
        assert_eq!(loaded_data[1], (1558253470, Some(1.70f64)));
        assert_eq!(loaded_data[5], (1558253474, Some(1.74f64)));
        // Let's add one more item and subtract one item from the array
        let test1_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "matrix",
                "result": [
                  {
                    "metric": {
                      "__name__": "node_load1",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "values": [
                        [1558253471,"1.71"],[1558253472,"1.72"],[1558253473,"1.73"],
                        [1558253474,"1.74"],[1558253475,"1.75"],[1558253476,"1.76"],
                        [1558253477,"1.77"],[1558253478,"1.78"],[1558253479,"1.79"],
                        [1558253480,"1.80"],[1558253481,"1.81"],[1558253482,"1.82"],
                        [1558253483,"1.83"],[1558253484,"1.84"],[1558253485,"1.85"],
                        [1558253486,"1.86"]]
                  }
                ]
              }
            }"#,
        );
        let res1_json = parse_json(&String::from("http://test"), &test1_json);
        assert!(res1_json.is_some());
        debug!("it_loads_prometheus_matrix NOTVEC: {:?}", test0.series.metrics);
        let loaded_data = test0.series.as_vec();
        debug!("it_loads_prometheus_matrix Data: {:?}", loaded_data);
        let res1_load = test0.load_prometheus_response(res1_json.clone().unwrap());
        // 7 items should have been loaded in the node_exporter, 9 already existed
        // 2 should have been rotated
        assert_eq!(res1_load, Ok(7usize));

        // Let's test reloading the data:
        let res1_load = test0.load_prometheus_response(res1_json.unwrap());
        // Now 0 records should have been loaded:
        assert_eq!(res1_load, Ok(0usize));
        debug!("it_loads_prometheus_matrix NOTVEC: {:?}", test0.series.metrics);
        let loaded_data = test0.series.metrics.clone();
        debug!("it_loads_prometheus_matrix Data: {:?}", loaded_data);
        assert_eq!(loaded_data[0], (1558253484, Some(1.84f64)));
        assert_eq!(loaded_data[3], (1558253472, Some(1.72f64)));
        assert_eq!(loaded_data[5], (1558253474, Some(1.74f64)));
        // This json is missing the value after the epoch
        let test2_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "matrix",
                "result": [
                  {
                    "metric": {
                      "__name__": "node_load1",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "values": [
                        [1558253478]
                    ]
                  }
                ]
              }
            }"#,
        );
        let res2_json = parse_json(&String::from("http://test"), &test2_json);
        assert!(res2_json.is_some());
        let res2_load = test0.load_prometheus_response(res2_json.unwrap());
        // 0 items should have been loaded, missing metric after epoch.
        assert_eq!(res2_load, Ok(0usize));
    }

    #[test]
    fn it_calculates_stats() {
        let metric_labels = HashMap::new();
        let test0_res: Result<PrometheusTimeSeries, String> = PrometheusTimeSeries::new(
            String::from("http://localhost:9090/api/v1/query?query=up"),
            15,
            String::from("vector"),
            metric_labels,
        );
        assert!(test0_res.is_ok());
        let mut test0 = test0_res.unwrap();
        let test1_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "matrix",
                "result": [
                  {
                    "metric": {
                      "__name__": "node_load1",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "values": [
                      [1566918913,"4.5"],
                      [1566918914,"4.5"],
                      [1566918915,"4.5"],
                      [1566918916,"4.5"],
                      [1566918917,"4.5"],
                      [1566918918,"4.5"],
                      [1566918919,"4.25"],
                      [1566918920,"4.25"],
                      [1566918921,"4.25"],
                      [1566918922,"4.25"],
                      [1566918923,"4.25"],
                      [1566918924,"4.25"],
                      [1566918925,"4"],
                      [1566918926,"4"],
                      [1566918927,"4"],
                      [1566918928,"4"],
                      [1566918929,"4"],
                      [1566918930,"4"],
                      [1566918931,"4.75"],
                      [1566918932,"4.75"],
                      [1566918933,"4.75"],
                      [1566918934,"4.75"],
                      [1566918935,"4.75"],
                      [1566918936,"4.75"]
                    ]
                  }
                ]
              }
            }"#,
        );
        let res1_json = parse_json(&String::from("http://test"), &test1_json);
        assert!(res1_json.is_some());
        let res1_load = test0.load_prometheus_response(res1_json.unwrap());
        // 1 items should have been loaded
        assert_eq!(res1_load, Ok(24usize));
        assert_eq!(
            test0.series.as_vec(),
            vec![
                (1566918913, Some(4.5)),
                (1566918914, Some(4.5)),
                (1566918915, Some(4.5)),
                (1566918916, Some(4.5)),
                (1566918917, Some(4.5)),
                (1566918918, Some(4.5)),
                (1566918919, Some(4.25)),
                (1566918920, Some(4.25)),
                (1566918921, Some(4.25)),
                (1566918922, Some(4.25)),
                (1566918923, Some(4.25)),
                (1566918924, Some(4.25)),
                (1566918925, Some(4.)),
                (1566918926, Some(4.)),
                (1566918927, Some(4.)),
                (1566918928, Some(4.)),
                (1566918929, Some(4.)),
                (1566918930, Some(4.)),
                (1566918931, Some(4.75)),
                (1566918932, Some(4.75)),
                (1566918933, Some(4.75)),
                (1566918934, Some(4.75)),
                (1566918935, Some(4.75)),
                (1566918936, Some(4.75))
            ]
        );
        test0.series.calculate_stats();
        let test0_sum = 4.5 * 6. + 4.25 * 6. + 4. * 6. + 4.75 * 6.;
        assert_eq!(
            test0.series.stats,
            TimeSeriesStats {
                first: 4.5,
                last: 4.75,
                count: 24,
                is_dirty: false,
                max: 4.75,
                min: 4.,
                sum: test0_sum,
                avg: test0_sum / 24.,
                last_epoch: 1566918936,
            }
        );
    }

    #[test]
    fn it_loads_prometheus_vector() {
        init_log();
        let mut metric_labels = HashMap::new();
        let test0_res: Result<PrometheusTimeSeries, String> = PrometheusTimeSeries::new(
            String::from("http://localhost:9090/api/v1/query?query=up"),
            15,
            String::from("vector"),
            metric_labels.clone(),
        );
        assert!(test0_res.is_ok());
        let mut test0 = test0_res.unwrap();
        // A json returned by prometheus
        let test0_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "vector",
                "result": [
                  {
                    "metric": {
                      "__name__": "up",
                      "instance": "localhost:9090",
                      "job": "prometheus"
                    },
                    "value": [
                      1557571137.732,
                      "1"
                    ]
                  },
                  {
                    "metric": {
                      "__name__": "up",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "value": [
                      1557571138.732,
                      "1"
                    ]
                  }
                ]
              }
            }"#,
        );
        let res0_json = parse_json(&String::from("http://test"), &test0_json);
        assert!(res0_json.is_some());
        let res0_load = test0.load_prometheus_response(res0_json.unwrap());
        // 2 items should have been loaded, one for Prometheus Server and the
        // other for Prometheus Node Exporter
        assert_eq!(res0_load, Ok(2usize));
        assert_eq!(
            test0.series.as_vec(),
            vec![(1557571137u64, Some(1.)), (1557571138u64, Some(1.))]
        );

        let test1_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "vector",
                "result": [
                  {
                    "metric": {
                      "__name__": "up",
                      "instance": "localhost:9090",
                      "job": "prometheus"
                    },
                    "value": [
                      1557571139.732,
                      "1"
                    ]
                  },
                  {
                    "metric": {
                      "__name__": "up",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "value": [
                      1557571140.732,
                      "1"
                    ]
                  }
                ]
              }
            }"#,
        );
        let res1_json = parse_json(&String::from("http://test"), &test1_json);
        assert!(res1_json.is_some());

        // Make the labels match only one instance
        metric_labels.insert(String::from("job"), String::from("prometheus"));
        metric_labels.insert(String::from("instance"), String::from("localhost:9090"));
        test0.required_labels = metric_labels.clone();
        let res1_load = test0.load_prometheus_response(res1_json.unwrap());
        // Only the prometheus: localhost:9090 should have been loaded with epoch 1557571139
        assert_eq!(res1_load, Ok(1usize));
        assert_eq!(
            test0.series.as_vec(),
            vec![(1557571137u64, Some(1.)), (1557571138u64, Some(1.)), (1557571139u64, Some(1.))]
        );

        let test2_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "vector",
                "result": [
                  {
                    "metric": {
                      "__name__": "up",
                      "instance": "localhost:9090",
                      "job": "prometheus"
                    },
                    "value": [
                      1557571141.732,
                      "1"
                    ]
                  },
                  {
                    "metric": {
                      "__name__": "up",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "value": [
                      1557571142.732,
                      "1"
                    ]
                  }
                ]
              }
            }"#,
        );
        let res2_json = parse_json(&String::from("http://test"), &test2_json);
        assert!(res2_json.is_some());
        // Make the labels not match
        metric_labels.insert(String::from("__name__"), String::from("down"));
        test0.required_labels = metric_labels;
        let res2_load = test0.load_prometheus_response(res2_json.unwrap());
        assert_eq!(res2_load, Ok(0usize));
        assert_eq!(
            test0.series.as_vec(),
            vec![(1557571137u64, Some(1.)), (1557571138u64, Some(1.)), (1557571139u64, Some(1.))]
        );
        // This json is missing the value after the epoch
        let test3_json = hyper::body::Bytes::from(
            r#"
            {
              "status": "success",
              "data": {
                "resultType": "vector",
                "result": [
                  {
                    "metric": {
                      "__name__": "node_load1",
                      "instance": "localhost:9100",
                      "job": "node_exporter"
                    },
                    "value": [
                        1558253478
                    ]
                  }
                ]
              }
            }"#,
        );
        let res3_json = parse_json(&String::from("http://test"), &test3_json);
        assert!(res3_json.is_some());
        let res3_load = test0.load_prometheus_response(res3_json.unwrap());
        // 0 items should have been loaded, the data is invalid
        assert_eq!(res3_load, Ok(0usize));
    }

    #[tokio::test]
    #[ignore]
    async fn it_gets_prometheus_metrics() {
        // These tests have been mocked above, but testing the actual communication
        // without creating a temporary web server is done needs this for now.
        init_log();
        let mut test_labels = HashMap::new();
        test_labels.insert(String::from("name"), String::from("up"));
        test_labels.insert(String::from("job"), String::from("prometheus"));
        test_labels.insert(String::from("instance"), String::from("localhost:9090"));
        // Test non plain http error:
        let test0_res: Result<PrometheusTimeSeries, String> = PrometheusTimeSeries::new(
            String::from("https://localhost:9090/api/v1/query?query=up"),
            15,
            String::from("vector"),
            test_labels.clone(),
        );
        assert_ne!(test0_res, Err(String::from("Unsupported protocol: Some(\"https\")")));
        let test1_res: Result<PrometheusTimeSeries, String> = PrometheusTimeSeries::new(
            String::from("http://localhost:9090/api/v1/query?query=up"),
            15,
            String::from("vector"),
            test_labels.clone(),
        );
        assert!(test1_res.is_ok());
        let test1 = test1_res.unwrap();
        let res1_get = tokio::try_join!(get_from_prometheus(test1.url.clone(), None));
        println!("get_from_prometheus: {:?}", res1_get);
        assert!(res1_get.is_ok());
        if let Some(prom_response) = parse_json(&String::from("http://test"), &res1_get.unwrap().0)
        {
            // This requires a Prometheus Server running locally
            // XXX: mock this.
            // Example playload:
            // {"status":"success","data":{"resultType":"vector","result":[
            //   {"metric":{"__name__":"up","instance":"localhost:9090","job":"prometheus"},
            //    "value":[1558270835.417,"1"]},
            //   {"metric":{"__name__":"up","instance":"localhost:9100","job":"node_exporter"},
            //    "value":[1558270835.417,"1"]}
            // ]}}
            assert_eq!(prom_response.status, String::from("success"));
            let mut found_prometheus_job_metric = false;
            if let HTTPResponseData::Vector { result: results } = prom_response.data {
                for prom_item in results.iter() {
                    if test1.match_metric_labels(&test_labels) {
                        assert_eq!(prom_item.value.len(), 2);
                        assert_eq!(prom_item.value[1], String::from("1"));
                        found_prometheus_job_metric = true;
                    }
                }
            }
            assert!(found_prometheus_job_metric);
        }
    }

    #[test]
    fn it_does_not_duplicate_epochs() {
        init_log();
        let test_labels = HashMap::new();
        let mut test = PrometheusTimeSeries {
            name: String::from("load average 1 min"),
            series: TimeSeries {
                metrics: vec![
                    (1571511822, Some(1.8359375)),
                    (1571511823, Some(1.8359375)),
                    (1571511824, Some(1.8359375)),
                    (1571511825, Some(1.8359375)),
                    (1571511826, Some(1.8359375)),
                ],
                metrics_capacity: 30,
                stats: TimeSeriesStats {
                    max: 17179869184.0,
                    min: 17179869184.0,
                    avg: 17179869184.0,
                    first: 17179869184.0,
                    last: 17179869184.0,
                    count: 5,
                    sum: 1202590842880.0,
                    is_dirty: false,
                    last_epoch: 1571511826,
                },
                collision_policy: ValueCollisionPolicy::Overwrite,
                missing_values_policy: MissingValuesPolicy::Zero,
                first_idx: 0,
                active_items: 5,
                prev_snapshot: vec![],
                prev_value: (1604568602, Some(6.0)),
                upsert_type: UpsertType::NewEpoch,
            },
            data: Vector {
                result: vec![HTTPVectorResult { labels: test_labels.clone(), value: vec![] }],
            },
            source: String::from(
                "http://localhost:9090/api/v1/query_range?query=node_memory_bytes_total",
            ),
            url: "/".parse::<hyper::Uri>().unwrap(),
            data_type: String::from(""),
            required_labels: test_labels,
            pull_interval: 15,
            color: Rgb { r: 207, g: 102, b: 121 },
            alpha: 1.0,
        };
        // This should result in adding 15 more items
        let test1_json = hyper::body::Bytes::from(
            r#"{
              "status":"success",
              "data":{
                "resultType":"matrix",
                "result":[{
                  "metric":{
                    "__name__":"node_load1",
                    "instance":"localhost:9100",
                    "job":"node_exporter"
                  },
                  "values":[
                    [1571511822,"1.8359322"],
                    [1571511823,"1.8359323"],
                    [1571511824,"1.8359324"],
                    [1571511825,"1.8359325"],
                    [1571511826,"1.8359326"],
                    [1571511827,"1.8359327"],
                    [1571511828,"1.8359328"],
                    [1571511829,"1.8359329"],
                    [1571511830,"1.8359330"],
                    [1571511831,"1.8359331"]
                  ]
                }]
              }
          }"#,
        );
        let res1_json = parse_json(&String::from("http://test"), &test1_json);
        assert!(res1_json.is_some());
        let res1_load = test.load_prometheus_response(res1_json.unwrap());
        // 5 items should have been loaded, 5 already existed.
        assert_eq!(res1_load, Ok(5usize));
        assert_eq!(test.series.active_items, 10usize);
        assert_eq!(
            test.series.as_vec(),
            vec![
                (1571511822, Some(1.8359322)),
                (1571511823, Some(1.8359323)),
                (1571511824, Some(1.8359324)),
                (1571511825, Some(1.8359325)),
                (1571511826, Some(1.8359326)),
                (1571511827, Some(1.8359327)),
                (1571511828, Some(1.8359328)),
                (1571511829, Some(1.8359329)),
                (1571511830, Some(1.8359330)),
                (1571511831, Some(1.8359331))
            ]
        );
    }

    #[test]
    fn it_does_not_lose_synchrony() {
        init_log();
        let test_labels = HashMap::new();
        let mut test = PrometheusTimeSeries {
            name: String::from("load average 5 min"),
            series: TimeSeries {
                metrics: vec![
                    (1583092654, None),
                    (1583091367, Some(5.5908203125)),
                    (1583091368, Some(5.5908203125)),
                    (1583091369, Some(5.5908203125)),
                    (1583091370, Some(5.5908203125)),
                    (1583091371, Some(5.5908203125)),
                    (1583091372, Some(5.5908203125)),
                    (1583091373, Some(5.5908203125)),
                    (1583091374, Some(5.5908203125)),
                    (1583091375, Some(5.5908203125)),
                    (1583091376, Some(5.5908203125)),
                    (1583091377, Some(5.5908203125)),
                    (1583091378, Some(5.3662109375)),
                    (1583091379, Some(5.3662109375)),
                    (1583091380, Some(5.3662109375)),
                    (1583091381, Some(5.3662109375)),
                    (1583091382, Some(5.3662109375)),
                    (1583091383, Some(5.3662109375)),
                    (1583091384, Some(5.3662109375)),
                    (1583091385, Some(5.3662109375)),
                    (1583091386, Some(5.3662109375)),
                    (1583091387, Some(5.3662109375)),
                    (1583091388, Some(5.3662109375)),
                    (1583091389, Some(5.3662109375)),
                    (1583091390, Some(5.3662109375)),
                    (1583091391, Some(5.3662109375)),
                    (1583091392, Some(5.3662109375)),
                    (1583091393, Some(5.427734375)),
                    (1583091394, Some(5.427734375)),
                    (1583091395, Some(5.427734375)),
                    (1583091396, Some(5.427734375)),
                    (1583091397, Some(5.427734375)),
                    (1583091398, Some(5.427734375)),
                    (1583091399, Some(5.427734375)),
                    (1583091400, Some(5.427734375)),
                    (1583091401, Some(5.427734375)),
                    (1583091402, Some(5.427734375)),
                    (1583091403, Some(5.427734375)),
                    (1583091404, Some(5.427734375)),
                    (1583091405, Some(5.427734375)),
                    (1583091406, Some(5.427734375)),
                    (1583091407, Some(5.427734375)),
                    (1583091408, Some(5.22607421875)),
                    (1583091409, Some(5.22607421875)),
                    (1583091410, Some(5.22607421875)),
                    (1583091411, Some(5.22607421875)),
                    (1583091412, Some(5.22607421875)),
                    (1583091413, Some(5.22607421875)),
                    (1583091414, Some(5.22607421875)),
                    (1583091415, Some(5.22607421875)),
                    (1583091416, Some(5.22607421875)),
                    (1583091417, Some(5.22607421875)),
                    (1583091418, Some(5.22607421875)),
                    (1583091419, Some(5.22607421875)),
                    (1583091420, Some(5.22607421875)),
                    (1583091421, Some(5.22607421875)),
                    (1583091422, Some(5.22607421875)),
                    (1583091423, Some(5.103515625)),
                    (1583091424, Some(5.103515625)),
                    (1583091425, Some(5.103515625)),
                    (1583091426, Some(5.103515625)),
                    (1583091427, Some(5.103515625)),
                    (1583091428, Some(5.103515625)),
                    (1583091429, Some(5.103515625)),
                    (1583091430, Some(5.103515625)),
                    (1583091431, Some(5.103515625)),
                    (1583091432, Some(5.103515625)),
                    (1583091433, Some(5.103515625)),
                    (1583091434, Some(5.103515625)),
                    (1583091435, Some(5.103515625)),
                    (1583091436, Some(5.103515625)),
                    (1583091437, Some(5.103515625)),
                    (1583091438, Some(5.08056640625)),
                    (1583091439, Some(5.08056640625)),
                    (1583091440, Some(5.08056640625)),
                    (1583091141, Some(8.09912109375)),
                    (1583091142, Some(8.09912109375)),
                    (1583091143, Some(8.09912109375)),
                    (1583091144, Some(8.09912109375)),
                    (1583091145, Some(8.09912109375)),
                    (1583091146, Some(8.09912109375)),
                    (1583091147, Some(8.09912109375)),
                    (1583091148, Some(8.09912109375)),
                    (1583091149, Some(8.09912109375)),
                    (1583091150, Some(8.09912109375)),
                    (1583091151, Some(8.09912109375)),
                    (1583091152, Some(8.09912109375)),
                    (1583091153, Some(7.78271484375)),
                    (1583091154, Some(7.78271484375)),
                    (1583091155, Some(7.78271484375)),
                    (1583091156, Some(7.78271484375)),
                    (1583091157, Some(7.78271484375)),
                    (1583091158, Some(7.78271484375)),
                    (1583091159, Some(7.78271484375)),
                    (1583091160, Some(7.78271484375)),
                    (1583091161, Some(7.78271484375)),
                    (1583091162, Some(7.78271484375)),
                    (1583091163, Some(7.78271484375)),
                    (1583091164, Some(7.78271484375)),
                    (1583091165, Some(7.78271484375)),
                    (1583091166, Some(7.78271484375)),
                    (1583091167, Some(7.78271484375)),
                    (1583091168, Some(7.49853515625)),
                    (1583091169, Some(7.49853515625)),
                    (1583091170, Some(7.49853515625)),
                    (1583091171, Some(7.49853515625)),
                    (1583091172, Some(7.49853515625)),
                    (1583091173, Some(7.49853515625)),
                    (1583091174, Some(7.49853515625)),
                    (1583091175, Some(7.49853515625)),
                    (1583091176, Some(7.49853515625)),
                    (1583091177, Some(7.49853515625)),
                    (1583091178, Some(7.49853515625)),
                    (1583091179, Some(7.49853515625)),
                    (1583091180, Some(7.49853515625)),
                    (1583091181, Some(7.49853515625)),
                    (1583091182, Some(7.49853515625)),
                    (1583091183, Some(7.16357421875)),
                    (1583091184, Some(7.16357421875)),
                    (1583091185, Some(7.16357421875)),
                    (1583091186, Some(7.16357421875)),
                    (1583091187, Some(7.16357421875)),
                    (1583091188, Some(7.16357421875)),
                    (1583091189, Some(7.16357421875)),
                    (1583091190, Some(7.16357421875)),
                    (1583091191, Some(7.16357421875)),
                    (1583091192, Some(7.16357421875)),
                    (1583091193, Some(7.16357421875)),
                    (1583091194, Some(7.16357421875)),
                    (1583091195, Some(7.16357421875)),
                    (1583091196, Some(7.16357421875)),
                    (1583091197, Some(7.16357421875)),
                    (1583091198, Some(6.9267578125)),
                    (1583091199, Some(6.9267578125)),
                    (1583091200, Some(6.9267578125)),
                    (1583091201, Some(6.9267578125)),
                    (1583091202, Some(6.9267578125)),
                    (1583091203, Some(6.9267578125)),
                    (1583091204, Some(6.9267578125)),
                    (1583091205, Some(6.9267578125)),
                    (1583091206, Some(6.9267578125)),
                    (1583091207, Some(6.9267578125)),
                    (1583091208, Some(6.9267578125)),
                    (1583091209, Some(6.9267578125)),
                    (1583091210, Some(6.9267578125)),
                    (1583091211, Some(6.9267578125)),
                    (1583091212, Some(6.9267578125)),
                    (1583091213, Some(6.701171875)),
                    (1583091214, Some(6.701171875)),
                    (1583091215, Some(6.701171875)),
                    (1583091216, Some(6.701171875)),
                    (1583091217, Some(6.701171875)),
                    (1583091218, Some(6.701171875)),
                    (1583091219, Some(6.701171875)),
                    (1583091220, Some(6.701171875)),
                    (1583091221, Some(6.701171875)),
                    (1583091222, Some(6.701171875)),
                    (1583091223, Some(6.701171875)),
                    (1583091224, Some(6.701171875)),
                    (1583091225, Some(6.701171875)),
                    (1583091226, Some(6.701171875)),
                    (1583091227, Some(6.701171875)),
                    (1583091228, Some(6.50244140625)),
                    (1583091229, Some(6.50244140625)),
                    (1583091230, Some(6.50244140625)),
                    (1583091231, Some(6.50244140625)),
                    (1583091232, Some(6.50244140625)),
                    (1583091233, Some(6.50244140625)),
                    (1583091234, Some(6.50244140625)),
                    (1583091235, Some(6.50244140625)),
                    (1583091236, Some(6.50244140625)),
                    (1583091237, Some(6.50244140625)),
                    (1583091238, Some(6.50244140625)),
                    (1583091239, Some(6.50244140625)),
                    (1583091240, Some(6.50244140625)),
                    (1583091241, Some(6.50244140625)),
                    (1583091242, Some(6.50244140625)),
                    (1583091243, Some(6.31298828125)),
                    (1583091244, Some(6.31298828125)),
                    (1583091245, Some(6.31298828125)),
                    (1583091246, Some(6.31298828125)),
                    (1583091247, Some(6.31298828125)),
                    (1583091248, Some(6.31298828125)),
                    (1583091249, Some(6.31298828125)),
                    (1583091250, Some(6.31298828125)),
                    (1583091251, Some(6.31298828125)),
                    (1583091252, Some(6.31298828125)),
                    (1583091253, Some(6.31298828125)),
                    (1583091254, Some(6.31298828125)),
                    (1583091255, Some(6.31298828125)),
                    (1583091256, Some(6.31298828125)),
                    (1583091257, Some(6.31298828125)),
                    (1583091258, Some(6.2666015625)),
                    (1583091259, Some(6.2666015625)),
                    (1583091260, Some(6.2666015625)),
                    (1583091261, Some(6.2666015625)),
                    (1583091262, Some(6.2666015625)),
                    (1583091263, Some(6.2666015625)),
                    (1583091264, Some(6.2666015625)),
                    (1583091265, Some(6.2666015625)),
                    (1583091266, Some(6.2666015625)),
                    (1583091267, Some(6.2666015625)),
                    (1583091268, Some(6.2666015625)),
                    (1583091269, Some(6.2666015625)),
                    (1583091270, Some(6.2666015625)),
                    (1583091271, Some(6.2666015625)),
                    (1583091272, Some(6.2666015625)),
                    (1583091273, Some(6.07177734375)),
                    (1583091274, Some(6.07177734375)),
                    (1583091275, Some(6.07177734375)),
                    (1583091276, Some(6.07177734375)),
                    (1583091277, Some(6.07177734375)),
                    (1583091278, Some(6.07177734375)),
                    (1583091279, Some(6.07177734375)),
                    (1583091280, Some(6.07177734375)),
                    (1583091281, Some(6.07177734375)),
                    (1583091282, Some(6.07177734375)),
                    (1583091283, Some(6.07177734375)),
                    (1583091284, Some(6.07177734375)),
                    (1583091285, Some(6.07177734375)),
                    (1583091286, Some(6.07177734375)),
                    (1583091287, Some(6.07177734375)),
                    (1583091288, Some(5.8720703125)),
                    (1583091289, Some(5.8720703125)),
                    (1583091290, Some(5.8720703125)),
                    (1583091291, Some(5.8720703125)),
                    (1583091292, Some(5.8720703125)),
                    (1583091293, Some(5.8720703125)),
                    (1583091294, Some(5.8720703125)),
                    (1583091295, Some(5.8720703125)),
                    (1583091296, Some(5.8720703125)),
                    (1583091297, Some(5.8720703125)),
                    (1583091298, Some(5.8720703125)),
                    (1583091299, Some(5.8720703125)),
                    (1583091300, Some(5.8720703125)),
                    (1583091301, Some(5.8720703125)),
                    (1583091302, Some(5.8720703125)),
                    (1583091303, Some(5.6494140625)),
                    (1583091304, Some(5.6494140625)),
                    (1583091305, Some(5.6494140625)),
                    (1583091306, Some(5.6494140625)),
                    (1583091307, Some(5.6494140625)),
                    (1583091308, Some(5.6494140625)),
                    (1583091309, Some(5.6494140625)),
                    (1583091310, Some(5.6494140625)),
                    (1583091311, Some(5.6494140625)),
                    (1583091312, Some(5.6494140625)),
                    (1583091313, Some(5.6494140625)),
                    (1583091314, Some(5.6494140625)),
                    (1583091315, Some(5.6494140625)),
                    (1583091316, Some(5.6494140625)),
                    (1583091317, Some(5.6494140625)),
                    (1583091318, Some(5.4853515625)),
                    (1583091319, Some(5.4853515625)),
                    (1583091320, Some(5.4853515625)),
                    (1583091321, Some(5.4853515625)),
                    (1583091322, Some(5.4853515625)),
                    (1583091323, Some(5.4853515625)),
                    (1583091324, Some(5.4853515625)),
                    (1583091325, Some(5.4853515625)),
                    (1583091326, Some(5.4853515625)),
                    (1583091327, Some(5.4853515625)),
                    (1583091328, Some(5.4853515625)),
                    (1583091329, Some(5.4853515625)),
                    (1583091330, Some(5.4853515625)),
                    (1583091331, Some(5.4853515625)),
                    (1583091332, Some(5.4853515625)),
                    (1583091333, Some(5.28125)),
                    (1583091334, Some(5.28125)),
                    (1583091335, Some(5.28125)),
                    (1583091336, Some(5.28125)),
                    (1583091337, Some(5.28125)),
                    (1583091338, Some(5.28125)),
                    (1583091339, Some(5.28125)),
                    (1583091340, Some(5.28125)),
                    (1583091341, Some(5.28125)),
                    (1583091342, Some(5.28125)),
                    (1583091343, Some(5.28125)),
                    (1583091344, Some(5.28125)),
                    (1583091345, Some(5.28125)),
                    (1583091346, Some(5.28125)),
                    (1583091347, Some(5.28125)),
                    (1583091348, Some(5.18505859375)),
                    (1583091349, Some(5.18505859375)),
                    (1583091350, Some(5.18505859375)),
                    (1583091351, Some(5.18505859375)),
                    (1583091352, Some(5.18505859375)),
                    (1583091353, Some(5.18505859375)),
                    (1583091354, Some(5.18505859375)),
                    (1583091355, Some(5.18505859375)),
                    (1583091356, Some(5.18505859375)),
                    (1583091357, Some(5.18505859375)),
                    (1583091358, Some(5.18505859375)),
                    (1583091359, Some(5.18505859375)),
                    (1583091360, Some(5.18505859375)),
                    (1583091361, Some(5.18505859375)),
                    (1583091362, Some(5.18505859375)),
                    (1583091363, Some(5.5908203125)),
                    (1583091364, Some(5.5908203125)),
                    (1583091365, Some(5.5908203125)),
                ],
                metrics_capacity: 300,
                stats: TimeSeriesStats {
                    max: 8.09912109375,
                    min: 5.08056640625,
                    avg: 6.147174479166667,
                    first: 8.09912109375,
                    last: 5.08056640625,
                    count: 300,
                    sum: 1844.15234375,
                    last_epoch: 1583091439,
                    is_dirty: false,
                },
                collision_policy: ValueCollisionPolicy::Overwrite,
                missing_values_policy: MissingValuesPolicy::Zero,
                first_idx: 0,
                active_items: 1,
                prev_snapshot: vec![],
                prev_value: (1604568602, Some(6.0)),
                upsert_type: UpsertType::NewEpoch,
            },
            data: Vector {
                result: vec![HTTPVectorResult { labels: test_labels.clone(), value: vec![] }],
            },
            source: String::from(
                "http://localhost:9090/api/v1/query_range?query=node_memory_bytes_total",
            ),
            url: "/".parse::<hyper::Uri>().unwrap(),
            data_type: String::from(""),
            required_labels: test_labels,
            pull_interval: 15,
            color: Rgb { r: 207, g: 102, b: 121 },
            alpha: 1.0,
        };
        assert_eq!(test.series.metrics.len(), 300usize);
        let test1_json = hyper::body::Bytes::from(
            r#"{
              "status":"success",
              "data":{
                "resultType":"matrix",
                "result":[{
                  "metric":{
                    "__name__":"node_load5",
                    "instance":"localhost:9100",
                    "job":"node_exporter"
                  },
                  "values":[
                    [1583092652, "5.0283203125"],
                    [1583092653, "5.0283203125"],
                    [1583092654, "5.0283203125"]
                ]
              }]
            }
          }"#,
        );
        let res1_json = parse_json(&String::from("http://test"), &test1_json);
        assert!(res1_json.is_some());
        let res1_load = test.load_prometheus_response(res1_json.unwrap());
        assert_eq!(res1_load, Ok(2usize));
        assert_eq!(test.series.active_items, 3usize);
        assert_eq!(test.series.metrics[0], (1583092654, Some(5.0283203125)));
        assert_eq!(test.series.metrics[299], (1583092653, Some(5.0283203125)));
        assert_eq!(test.series.metrics[298], (1583092652, Some(5.0283203125)));
        assert_eq!(test.series.first_idx, 298usize);
        assert_eq!(test.series.active_items, 3usize);
        assert_eq!(
            test.series.as_vec(),
            vec![
                (1583092652, Some(5.0283203125)),
                (1583092653, Some(5.0283203125)),
                (1583092654, Some(5.0283203125))
            ]
        );
    }
}
