use std::collections::HashMap;

use serde::Deserialize;

use super::YahooError;

#[derive(Deserialize, Debug)]
pub struct YResponse {
    pub chart: YChart,
}

impl YResponse {
    fn check_consistency(&self) -> Result<(), YahooError> {
        for stock in &self.chart.result {
            let n = stock.timestamp.len();
            if n == 0 {
                return Err(YahooError::EmptyDataSet);
            }
            let quote = &stock.indicators.quote[0];
            if quote.opens().len() != n
                || quote.highs().len() != n
                || quote.lows().len() != n
                || quote.volumes().len() != n
                || quote.closes().len() != n
            {
                eprintln!("count timestamps = {}, opens = {}", n, quote.opens().len());
                return Err(YahooError::DataInconsistency);
            }
            if let Some(ref adjclose) = stock.indicators.adjclose {
                if adjclose[0].adjcloses().len() != n {
                    return Err(YahooError::DataInconsistency);
                }
            }
        }
        Ok(())
    }

    pub fn from_json(json: serde_json::Value) -> Result<YResponse, YahooError> {
        //let s = json.clone();
        serde_json::from_value(json).map_err(|e| {
            //eprintln!(">>>>>> WTF RESPONSE: {:?}", serde_json::to_string(&s));
            YahooError::DeserializeFailed(format!("YResponse: {}", e))
        })
    }

    /// Return the latest valid quote
    pub fn last_quote(&self) -> Result<Quote, YahooError> {
        self.check_consistency()?;
        let stock = &self.chart.result[0];
        let n = stock.timestamp.len();
        for i in (0..n).rev() {
            let quote = stock.indicators.get_ith_quote(stock.timestamp[i], i);
            if quote.is_ok() {
                return quote;
            }
        }
        Err(YahooError::EmptyDataSet)
    }

    pub fn quotes(&self) -> Result<Vec<Quote>, YahooError> {
        self.check_consistency()?;
        let stock = &self.chart.result[0];
        let mut quotes = Vec::new();
        let n = stock.timestamp.len();
        for i in 0..n {
            let timestamp = stock.timestamp[i];
            let quote = stock.indicators.get_ith_quote(timestamp, i);
            if let Ok(q) = quote {
                quotes.push(q);
            }
        }
        Ok(quotes)
    }

    /// This method retrieves information about the splits that might have
    /// occured during the considered time period
    pub fn splits(&self) -> Result<Vec<Split>, YahooError> {
        self.check_consistency()?;
        let stock = &self.chart.result[0];
        if let Some(events) = &stock.events {
            if let Some(splits) = &events.splits {
                let mut data = splits.values().cloned().collect::<Vec<Split>>();
                data.sort_unstable_by_key(|d| d.date);
                return Ok(data);
            }
        }
        Ok(vec![])
    }
    /// This method retrieves information about the dividends that have
    /// been recorded during the considered time period.
    ///
    /// Note: Date is the ex-dividend date)
    pub fn dividends(&self) -> Result<Vec<Dividend>, YahooError> {
        self.check_consistency()?;
        let stock = &self.chart.result[0];
        if let Some(events) = &stock.events {
            if let Some(dividends) = &events.dividends {
                let mut data = dividends.values().cloned().collect::<Vec<Dividend>>();
                data.sort_unstable_by_key(|d| d.date);
                return Ok(data);
            }
        }
        Ok(vec![])
    }
}

/// Struct for single quote
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Quote {
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: u64,
    pub close: f64,
    pub adjclose: f64,
}

#[derive(Deserialize, Debug)]
pub struct YChart {
    pub result: Vec<YQuoteBlock>,
    pub error: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct YQuoteBlock {
    pub meta: YMetaData,
    #[serde(default)]
    pub timestamp: Vec<u64>,
    pub events: Option<EventsBlock>,
    pub indicators: QuoteBlock,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YMetaData {
    pub currency: Option<String>,
    pub symbol: String,
    pub exchange_name: String,
    pub instrument_type: String,
    pub first_trade_date: i32,
    pub regular_market_time: u32,
    pub gmtoffset: i32,
    pub timezone: String,
    pub exchange_timezone_name: String,
    pub regular_market_price: f64,
    pub chart_previous_close: f64,
    #[serde(default)]
    pub previous_close: Option<f64>,
    #[serde(default)]
    pub scale: Option<i32>,
    pub price_hint: i32,
    pub current_trading_period: TradingPeriod,
    #[serde(default)]
    pub trading_periods: Option<Vec<Vec<PeriodInfo>>>,
    pub data_granularity: String,
    pub range: String,
    pub valid_ranges: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct TradingPeriod {
    pub pre: PeriodInfo,
    pub regular: PeriodInfo,
    pub post: PeriodInfo,
}

#[derive(Deserialize, Debug)]
pub struct PeriodInfo {
    pub timezone: String,
    pub start: u32,
    pub end: u32,
    pub gmtoffset: i32,
}

#[derive(Deserialize, Debug)]
pub struct QuoteBlock {
    quote: Vec<QuoteList>,
    #[serde(default)]
    adjclose: Option<Vec<AdjClose>>,
}

impl QuoteBlock {
    fn get_ith_quote(&self, timestamp: u64, i: usize) -> Result<Quote, YahooError> {
        let adjclose = match &self.adjclose {
            Some(adjclose) => adjclose[0].adjclose(i),
            None => None,
        };
        let quote = &self.quote[0];
        if quote.close(i).is_none() {
            return Err(YahooError::EmptyDataSet);
        }
        Ok(Quote {
            timestamp,
            open: quote.open(i).unwrap_or(0.0),
            high: quote.high(i).unwrap_or(0.0),
            low: quote.low(i).unwrap_or(0.0),
            volume: quote.volume(i).unwrap_or(0),
            close: quote.close(i).unwrap(),
            adjclose: adjclose.unwrap_or(0.0),
        })
    }
}

/// A stand-in for weird `[{}]` encoded things
#[derive(Deserialize, Debug)]
pub struct Nothing {
    pub nada: Option<bool>,
}

macro_rules! actualize {
    ($self:ident) => {
        pub fn is_actual(&self) -> bool {
            match self {
                $self::Empty(_) => false,
                $self::Actual(_) => true,
            }
        }
    };
}

macro_rules! actual_attribute {
    ($self:ident, $attribs:ident, $attrib:ident, $type:ty) => {
        pub fn $attribs(&self) -> Vec<Option<$type>> {
            match self {
                $self::Empty(_) => vec![],
                $self::Actual(actual) => actual.$attrib.to_owned(),
            }
        }

        pub fn $attrib(&self, i: usize) -> Option<$type> {
            match self {
                $self::Empty(_) => None,
                $self::Actual(actual) => {
                    if i < actual.$attrib.len() {
                        actual.$attrib[i]
                    } else {
                        None
                    }
                }
            }
        }
    };
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum AdjClose {
    Actual(AdjCloseActual),
    Empty(Nothing),
}

impl AdjClose {
    actualize!(AdjClose);
    actual_attribute!(AdjClose, adjcloses, adjclose, f64);
}

#[derive(Deserialize, Debug)]
pub struct AdjCloseActual {
    adjclose: Vec<Option<f64>>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum QuoteList {
    Actual(QuoteListActual),
    Empty(Nothing),
}

#[derive(Deserialize, Debug)]
pub struct QuoteListActual {
    pub volume: Vec<Option<u64>>,
    pub high: Vec<Option<f64>>,
    pub close: Vec<Option<f64>>,
    pub low: Vec<Option<f64>>,
    pub open: Vec<Option<f64>>,
}

impl QuoteList {
    actualize!(QuoteList);
    actual_attribute!(QuoteList, volumes, volume, u64);
    actual_attribute!(QuoteList, highs, high, f64);
    actual_attribute!(QuoteList, closes, close, f64);
    actual_attribute!(QuoteList, lows, low, f64);
    actual_attribute!(QuoteList, opens, open, f64);
}

#[derive(Deserialize, Debug)]
pub struct EventsBlock {
    pub splits: Option<HashMap<u64, Split>>,
    pub dividends: Option<HashMap<u64, Dividend>>,
}

/// This structure simply models a split that has occured.
#[derive(Deserialize, Debug, Clone)]
pub struct Split {
    /// This is the date (timestamp) when the split occured
    pub date: u64,
    /// Numerator of the split. For instance a 1:5 split means you get 5 share
    /// wherever you had one before the split. (Here the numerator is 1 and
    /// denom is 5). A reverse split is considered as nothing but a regular
    /// split with a numerator > denom.
    pub numerator: u64,
    /// Denominator of the split. For instance a 1:5 split means you get 5 share
    /// wherever you had one before the split. (Here the numerator is 1 and
    /// denom is 5). A reverse split is considered as nothing but a regular
    /// split with a numerator > denom.
    pub denominator: u64,
    /// A textual representation of the split.
    #[serde(rename = "splitRatio")]
    pub split_ratio: String,
}

/// This structure simply models a dividend which has been recorded.
#[derive(Deserialize, Debug, Clone)]
pub struct Dividend {
    /// This is the price of the dividend
    pub amount: f64,
    /// This is the ex-dividend date
    pub date: u64,
}
