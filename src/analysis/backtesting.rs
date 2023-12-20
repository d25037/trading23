use super::live::Ohlc;
use crate::my_error::MyError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum LongShortControl {
    Long,
    Short,
    Control,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestAnalyzer {
    date: String,
    standardized_diff: f64,
    day5_with_stop_loss_38: f64,
    day5_with_stop_loss_50: f64,
    day5_with_stop_loss_62: f64,
    day10_with_stop_loss_38: f64,
    day10_with_stop_loss_50: f64,
    day10_with_stop_loss_62: f64,
    day20_with_stop_loss_38: f64,
    day20_with_stop_loss_50: f64,
    day20_with_stop_loss_62: f64,
    long_or_short_or_control: LongShortControl,
}

impl BacktestAnalyzer {
    pub fn new(raw_ohlc: Vec<Ohlc>, day: usize) -> Result<Self, MyError> {
        let testing_ohlc_60 = &raw_ohlc[day..(day + 60)];
        let testing_ohlc_20 = &raw_ohlc[(day + 40)..(day + 60)];
        let future_ohlc_20 = &raw_ohlc[(day + 60)..(day + 81)];
        let future_ohlc_10 = &raw_ohlc[(day + 60)..(day + 71)];

        let date = testing_ohlc_60[59].get_date();

        let (prev_19, last) = testing_ohlc_20.split_at(19);
        let last_close = last[0].get_close();
        let high = prev_19
            .iter()
            .map(|ohlc| ohlc.get_high())
            .fold(f64::NAN, f64::max);
        let low = prev_19
            .iter()
            .map(|ohlc| ohlc.get_low())
            .fold(f64::NAN, f64::min);

        let long_or_short_or_control = match (last_close > high, last_close < low) {
            (true, _) => LongShortControl::Long,
            (_, true) => LongShortControl::Short,
            _ => LongShortControl::Control,
        };

        let stop_loss_range_38 = match long_or_short_or_control {
            LongShortControl::Long => (last_close - low) * 0.38,
            LongShortControl::Short => (high - last_close) * 0.38,
            LongShortControl::Control => (high - low) * 0.38,
        };

        let highest_high = testing_ohlc_60
            .iter()
            .map(|ohlc| ohlc.get_high())
            .fold(f64::NAN, f64::max);
        let lowest_low = testing_ohlc_60
            .iter()
            .map(|ohlc| ohlc.get_low())
            .fold(f64::NAN, f64::min);

        let diff_sum: f64 = testing_ohlc_60
            .iter()
            .map(|ohlc| ohlc.get_high() - ohlc.get_low())
            .sum();
        let average_diff = diff_sum / testing_ohlc_60.len() as f64;

        let standardized_diff =
            (average_diff / (highest_high - lowest_low) * 1000.0).trunc() / 1000.0;

        let mut ohlc_vec = testing_ohlc_60.to_vec();
        for i in 0..testing_ohlc_60.len() - 1 {
            ohlc_vec[i + 1].set_open(testing_ohlc_60[i].get_close());
            if ohlc_vec[i + 1].get_open() > ohlc_vec[i + 1].get_high() {
                ohlc_vec[i + 1].set_high(testing_ohlc_60[i].get_close());
            }
            if ohlc_vec[i + 1].get_open() < ohlc_vec[i + 1].get_low() {
                ohlc_vec[i + 1].set_low(testing_ohlc_60[i].get_close());
            }
        }

        fn day_x_close(
            day_x: usize,
            future_ohlc: &[Ohlc],
            long_or_short_or_control: &LongShortControl,
            stop_loss_range: f64,
        ) -> f64 {
            let result =
                (future_ohlc[day_x].get_close() - future_ohlc[0].get_open()) / stop_loss_range;
            let result_rounded = (result * 100.0).round() / 100.0;
            match long_or_short_or_control {
                LongShortControl::Long | LongShortControl::Control => result_rounded,
                LongShortControl::Short => result_rounded * -1.0,
            }
        }

        fn day_x_with_stop_loss(
            day_x: usize,
            future_ohlc: &[Ohlc],
            stop_loss_range: f64,
            long_or_short_or_control: &LongShortControl,
        ) -> f64 {
            match long_or_short_or_control {
                LongShortControl::Long | LongShortControl::Control => match future_ohlc
                    .iter()
                    .map(|x| (x.get_low() - future_ohlc[0].get_open()) / stop_loss_range)
                    .fold(f64::NAN, f64::min)
                    < -1.0
                {
                    true => -1.0,
                    false => day_x_close(
                        day_x,
                        future_ohlc,
                        long_or_short_or_control,
                        stop_loss_range,
                    ),
                },
                LongShortControl::Short => match future_ohlc
                    .iter()
                    .map(|x| (x.get_high() - future_ohlc[0].get_open()) / stop_loss_range)
                    .fold(f64::NAN, f64::max)
                    > 1.0
                {
                    true => -1.0,
                    false => day_x_close(
                        day_x,
                        future_ohlc,
                        long_or_short_or_control,
                        stop_loss_range,
                    ),
                },
            }
        }

        let day5_with_stop_loss_38 = day_x_with_stop_loss(
            4,
            future_ohlc_10,
            stop_loss_range_38,
            &long_or_short_or_control,
        );

        let day10_with_stop_loss_38 = day_x_with_stop_loss(
            9,
            future_ohlc_10,
            stop_loss_range_38,
            &long_or_short_or_control,
        );

        let day20_with_stop_loss_38 = day_x_with_stop_loss(
            19,
            future_ohlc_20,
            stop_loss_range_38,
            &long_or_short_or_control,
        );

        let stop_loss_range_50 = match long_or_short_or_control {
            LongShortControl::Long => (last_close - low) * 0.5,
            LongShortControl::Short => (high - last_close) * 0.5,
            LongShortControl::Control => (high - low) * 0.5,
        };

        let day5_with_stop_loss_50 = day_x_with_stop_loss(
            4,
            future_ohlc_10,
            stop_loss_range_50,
            &long_or_short_or_control,
        );

        let day10_with_stop_loss_50 = day_x_with_stop_loss(
            9,
            future_ohlc_10,
            stop_loss_range_50,
            &long_or_short_or_control,
        );

        let day20_with_stop_loss_50 = day_x_with_stop_loss(
            19,
            future_ohlc_20,
            stop_loss_range_50,
            &long_or_short_or_control,
        );

        let stop_loss_range_62 = match long_or_short_or_control {
            LongShortControl::Long => (last_close - low) * 0.62,
            LongShortControl::Short => (high - last_close) * 0.62,
            LongShortControl::Control => (high - low) * 0.62,
        };

        let day5_with_stop_loss_62 = day_x_with_stop_loss(
            4,
            future_ohlc_10,
            stop_loss_range_62,
            &long_or_short_or_control,
        );

        let day10_with_stop_loss_62 = day_x_with_stop_loss(
            9,
            future_ohlc_10,
            stop_loss_range_62,
            &long_or_short_or_control,
        );

        let day20_with_stop_loss_62 = day_x_with_stop_loss(
            19,
            future_ohlc_20,
            stop_loss_range_62,
            &long_or_short_or_control,
        );

        Ok(Self {
            date: date.to_string(),
            standardized_diff,
            // standardized_diff_closed_window,
            day5_with_stop_loss_38,
            day5_with_stop_loss_50,
            day5_with_stop_loss_62,
            day10_with_stop_loss_38,
            day10_with_stop_loss_50,
            day10_with_stop_loss_62,
            day20_with_stop_loss_38,
            day20_with_stop_loss_50,
            day20_with_stop_loss_62,
            long_or_short_or_control,
        })
    }
}

// #[allow(dead_code)]
// pub fn aaa() {
//     let df = CsvReader::from_path("./jquants_backtest.csv")
//         .unwrap()
//         .finish()
//         .unwrap()
//         .group_by(["long_or_short_or_control"])
//         .unwrap()
//         .select(["day5_close", "day6_open", "day10_close", "day11_open"])
//         .mean();

//     info!("{:?}", df);
// }
