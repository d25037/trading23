import os
from enum import Enum, auto

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd


class AssetClass(Enum):
    FX = auto()
    STOCK = auto()


def read_csv_as_pd(file_name: str) -> pd.DataFrame:
    gdrive_path = os.environ.get("GDRIVE_PATH")
    if gdrive_path is None:
        raise ValueError("GDRIVE_PATH is not set")
    csv_path = os.path.join(gdrive_path, "trading23", file_name)
    df = pd.read_csv(csv_path)
    print(f"len(df): {len(df)}")
    column_dict = {}
    for i, column in enumerate(df.columns):
        column_dict[i] = column
    print(f"columns: {column_dict}")
    return df.sort_values("date")


def filter_by_long_short_control(df: pd.DataFrame):
    long = df.query("long_or_short_or_control == 'Long'")
    short = df.query("long_or_short_or_control == 'Short'")
    control = df.query("long_or_short_or_control == 'Control'")
    without_control = df.query("long_or_short_or_control != 'Control'")

    print(
        f"len(long): {len(long)}, len(short): {len(short)}, len(control): {len(control)}"
    )
    for arg in [
        "day10_with_stop_loss",
        "day10_with_stop_loss_2",
        "day10_with_stop_loss_3",
        "day20_with_stop_loss",
        "day20_with_stop_loss_2",
        "day20_with_stop_loss_3",
    ]:
        print("----------------------------------")
        print(f"{arg}")
        print(f"long: {my_round(long[arg].mean())}")
        print(f"short: {my_round(short[arg].mean())}")
        print(f"control: {my_round(control[arg].mean())}")
    return (long, short, control, without_control)


def filter_by_standardized_diff(
    name: str,
    df: pd.DataFrame,
    threshold_a: float,
    threshold_b: float,
    close_window: bool = False,
):
    if threshold_a > threshold_b:
        raise ValueError("threshold_a must be less than threshold_b")

    if close_window:
        standardized_diff = "standardized_diff_closed_window"
    else:
        standardized_diff = "standardized_diff"

    print("--------------------------------------------------")
    print(f"name: {name}, threshold_a: {threshold_a}, threshold_b: {threshold_b}")

    strong = df.query(f"{standardized_diff} < {threshold_a}")
    middle = df.query(f"{threshold_a} <= {standardized_diff} < {threshold_b}")
    weak = df.query(f"{threshold_b} <= {standardized_diff}")

    print(
        f"name: {name}, len(strong): {len(strong)}, len(middle): {len(middle)}, len(weak): {len(weak)}"
    )
    for arg in [
        "day10_with_stop_loss",
        "day10_with_stop_loss_2",
        "day10_with_stop_loss_3",
        "day20_with_stop_loss",
        "day20_with_stop_loss_2",
        "day20_with_stop_loss_3",
    ]:
        stop_loss_len_strong = len(strong.query(f"{arg} == -1"))
        stop_loss_len_middle = len(middle.query(f"{arg} == -1"))
        stop_loss_len_weak = len(weak.query(f"{arg} == -1"))

        print("----------------------------------")
        print(f"name: {name}, arg: {arg}")
        print(
            f"strong: {my_round(strong[f'{arg}'].mean())}, --- stop loss: {int(100 * (stop_loss_len_strong / len(strong)))}%"
        )
        print(
            f"middle: {my_round(middle[f'{arg}'].mean())}, --- stop loss: {int(100 * (stop_loss_len_middle / len(middle)))}%"
        )
        print(
            f"weak: {my_round(weak[f'{arg}'].mean())}, --- stop loss: {int(100 * (stop_loss_len_weak / len(weak)))}%"
        )
    return {"strong": strong, "middle": middle, "weak": weak}


def _analyze_settlement_order(dict: dict[str, pd.DataFrame]):
    for key, value in dict.items():
        print(f"key: {key}")
        print(
            f"day6_open - day5_close: {my_round((value['day6_open'] - value['day5_close']).mean())}"
        )
        print(
            f"day11_open - day10_close: {my_round((value['day11_open'] - value['day10_close']).mean())}"
        )

        success_rate = 0.5
        pre = "day5_close_stop_loss"
        post = "day10_close_stop_loss"
        print(f"success_rate: {success_rate}")
        df_success = value.query(f"{pre} > {success_rate}")
        day10_day5_diff = df_success[f"{post}"] - df_success[f"{pre}"]
        print(
            f"len(df_success): {len(df_success)}, day10_day5_diff.mean(): {my_round(day10_day5_diff.mean())}"
        )
    return


def plot_settlement_order(df: pd.DataFrame):
    x = np.linspace(0, 1, 10)
    y = []
    for i in x:
        success = df.query(f"day5_close > {i}")
        day10_day5_diff = (
            success["day10_close_stop_loss"] - success["day5_close_stop_loss"]
        )
        y.append(day10_day5_diff.mean())

    plt.plot(x, y)
    plt.show()


def daily_long_short(df: pd.DataFrame) -> dict[str, str]:
    date = "2011-01-01"
    i = 0
    x = []
    y = []

    entry_date = {}
    for index, row in df.iterrows():
        if row["date"] == date:
            continue

        date = row["date"]
        filtered_df = df.query(f"date == '{date}'")
        count_result = filtered_df["long_or_short_or_control"].value_counts()
        try:
            count_long = count_result["Long"]
        except KeyError:
            count_long = 0
        try:
            count_short = count_result["Short"]
        except KeyError:
            count_short = 0
        long_minus_short = count_long - count_short
        if long_minus_short > 14:
            entry_date[date] = "Long"
        elif long_minus_short < -14:
            entry_date[date] = "Short"

        y.append(long_minus_short)
        x.append(i)

        if i % 20 == 0:
            print(f"i: {i}, date: {date}, long_minus_short: {long_minus_short}")
        i += 1

    # series = pd.Series(y)
    # print(series.describe())

    # plt.plot(x, y)
    # plt.show()
    return entry_date


def backtest(df: pd.DataFrame, entry_date: dict[str, str]):
    x = []
    y = []
    sum = 0.0
    for i, (date, long_or_short) in enumerate(entry_date.items()):
        if i % 10 == 0:
            print(f"i: {i}, date: {date}")
        x.append(i)
        filtered_df = df.query(
            f"date == '{date}' and long_or_short_or_control == '{long_or_short}'"
        )
        # re_filtered_df_a = filtered_df.query("day5_close_stop_loss > 1")
        # re_filtered_df_b = filtered_df.query("day5_close_stop_loss <= 1")
        # sum_a, n_a = (
        #     re_filtered_df_a["day10_close_stop_loss"].sum(),
        #     len(re_filtered_df_a),
        # )
        # sum_b, n_b = (
        #     re_filtered_df_b["day5_close_stop_loss"].sum(),
        #     len(re_filtered_df_b),
        # )
        # result = (sum_a + sum_b) / (n_a + n_b)
        re_filtered_df = filtered_df.query("standardized_diff_closed_window > 0.12 ")
        result = re_filtered_df["day20_with_stop_loss_3"].mean()
        sum += result
        print(f"sum: {sum}")
        y.append(sum)

    plt.plot(x, y)
    plt.show()
    return


def my_round(value: float | int | None) -> float | int | None:
    if isinstance(value, float):
        return round(value, 3)
    return value
