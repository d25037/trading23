import os

import matplotlib.pyplot as plt
import numpy as np
import polars as pl
from pandas_analysis import my_round


def read_json_as_pd(file_name: str) -> pl.DataFrame:
    gdrive_path = os.environ.get("GDRIVE_PATH")
    if gdrive_path is None:
        raise ValueError("GDRIVE_PATH is not set")
    json_path = os.path.join(gdrive_path, "trading23", "backtest_json", file_name)
    df = pl.read_json(json_path)
    print(f"len(lf): {df.select(pl.count())}")
    print(f"columns: {df.columns}")
    return df.sort("date")


def filter_by_long_short_control(df: pl.DataFrame):
    long = df.filter(pl.col("long_or_short_or_control") == "Long")
    short = df.filter(pl.col("long_or_short_or_control") == "Short")
    control = df.filter(pl.col("long_or_short_or_control") == "Control")
    without_control = df.filter(pl.col("long_or_short_or_control") != "Control")

    print(
        f"len(long): {len(long)}, len(short): {len(short)}, len(control): {len(control)}"
    )
    for arg in [
        "day5_with_stop_loss_38",
        "day5_with_stop_loss_50",
        "day5_with_stop_loss_62",
        "day10_with_stop_loss_38",
        "day10_with_stop_loss_50",
        "day10_with_stop_loss_62",
        "day20_with_stop_loss_38",
        "day20_with_stop_loss_50",
        "day20_with_stop_loss_62",
    ]:
        print("----------------------------------")
        print(f"{arg}")
        print(f"long: {my_round(long.get_column(arg).mean())}")
        print(f"short: {my_round(short.get_column(arg).mean())}")
        print(f"control: {my_round(control.get_column(arg).mean())}")
    return (long, short, control, without_control)


def filter_by_standardized_diff(
    name: str,
    df: pl.DataFrame,
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

    strong = df.filter(pl.col("standardized_diff") < threshold_a)
    middle = df.filter(pl.col("standardized_diff").is_between(threshold_a, threshold_b))
    weak = df.filter(pl.col("standardized_diff") > threshold_b)

    print(
        f"name: {name}, len(strong): {len(strong)}, len(middle): {len(middle)}, len(weak): {len(weak)}"
    )
    for arg in [
        # "day5_with_stop_loss_38",
        # "day5_with_stop_loss_50",
        # "day5_with_stop_loss_62",
        "day10_with_stop_loss_38",
        "day10_with_stop_loss_50",
        # "day10_with_stop_loss_62",
        "day20_with_stop_loss_38",
        "day20_with_stop_loss_50",
        # "day20_with_stop_loss_62",
    ]:
        stop_loss_len_strong = len(strong.filter(pl.col(arg) == -1))
        stop_loss_len_middle = len(middle.filter(pl.col(arg) == -1))
        stop_loss_len_weak = len(weak.filter(pl.col(arg) == -1))

        minus_len_strong = len(strong.filter(pl.col(arg) < 0))
        minus_len_middle = len(middle.filter(pl.col(arg) < 0))
        minus_len_weak = len(weak.filter(pl.col(arg) < 0))

        print("----------------------------------")
        print(f"name: {name}, arg: {arg}")
        print(
            f"strong: {my_round(strong.get_column(arg).mean())}, --- stop loss: {int(100 * (stop_loss_len_strong / len(strong)))}%, --- minus: {int(100 * (minus_len_strong / len(strong)))}%"
        )
        print(
            f"middle: {my_round(middle.get_column(arg).mean())}, --- stop loss: {int(100 * (stop_loss_len_middle / len(middle)))}%, --- minus: {int(100 * (minus_len_middle / len(middle)))}%"
        )
        print(
            f"weak: {my_round(weak.get_column(arg).mean())}, --- stop loss: {int(100 * (stop_loss_len_weak / len(weak)))}%, --- minus: {int(100 * (minus_len_weak / len(weak)))}%"
        )
    return {"strong": strong, "middle": middle, "weak": weak}


def group_by_diff(df: pl.DataFrame):
    print(len(df))
    df = df.with_columns((pl.col("standardized_diff")).round(2).alias("rounded_diff"))
    # print(df["rounded_diff"].value_counts().sort("rounded_diff"))

    for arg in [
        # "day5_with_stop_loss_38",
        # "day5_with_stop_loss_50",
        # "day5_with_stop_loss_62",
        "day10_with_stop_loss_38",
        "day10_with_stop_loss_50",
        # "day10_with_stop_loss_62",
        "day20_with_stop_loss_38",
        "day20_with_stop_loss_50",
        # "day20_with_stop_loss_62",
    ]:
        df_2 = df.groupby("rounded_diff").agg(pl.mean(arg))
        # print(df_2.sort("rounded_diff"))

        # plot df_2
        x = df_2.get_column("rounded_diff").to_numpy()
        y = df_2.get_column(arg).to_numpy()
        plt.scatter(x, y)
        plt.title(arg)  # グラフのタイトルを設定
        plt.xlabel("rounded_diff")  # x軸のラベルを設定
        plt.ylabel(arg)
        plt.show()


def plot_cum_sum(df: pl.DataFrame, column_name: str):
    # df = df.filter(pl.col("standardized_diff").is_between(0.11, 0.13))
    cum_summed_series = df.get_column(column_name).cum_sum()
    x = df.get_column("date").to_numpy()

    plt.plot(x, cum_summed_series.to_numpy())

    selected_indices = np.linspace(0, len(x) - 1, 3, dtype=int)
    selected_labels = [str(x[i]) for i in selected_indices]  # Convert labels to strings
    plt.xticks(selected_indices, selected_labels)

    plt.show()
