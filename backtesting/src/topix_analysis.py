import os

import polars as pl


def read_json_as_pd(file_name: str) -> pl.DataFrame:
    gdrive_path = os.environ.get("GDRIVE_PATH")
    if gdrive_path is None:
        raise ValueError("GDRIVE_PATH is not set")
    json_path = os.path.join(gdrive_path, "trading23", "backtest_json", file_name)
    df = pl.read_json(json_path)
    print(f"len(lf): {df.select(pl.count())}")
    print(f"columns: {df.columns}")
    print(df.head())
    return df.sort("date")


def analysis(df: pl.DataFrame):
    df = df.with_columns(
        ((pl.col("open") / pl.col("last_close")).round(3)).alias("open_diff"),
        ((pl.col("close") / pl.col("open")).round(3)).alias("today_diff"),
    )
    print("open_diff")
    print(df.get_column("open_diff").describe())
    print("today_diff")
    print(df.get_column("today_diff").describe())
    long_df = df.filter(pl.col("open_diff") > 1.0)
    short_df = df.filter(pl.col("today_diff") < 1.0)

    print(f"all len: {df.select(pl.count())}")
    print(df.group_by("weekday").agg((pl.col("today_diff")).mean()).sort("weekday"))

    print("long len: ", long_df.select(pl.count()))
    print(
        long_df.group_by("weekday").agg((pl.col("today_diff")).mean()).sort("weekday")
    )

    print("short len: ", short_df.select(pl.count()))
    print(
        short_df.group_by("weekday").agg((pl.col("today_diff")).mean()).sort("weekday")
    )

    result = df.select(["date", "open_diff", "today_diff", "weekday"])
    print(result.tail(10))

    return result


def df_stocks_modifier(df: pl.DataFrame):
    return df.select(
        [
            "date",
            "code",
            "standardized_diff",
            "day1_with_stop_loss_38",
            "long_or_short_or_control",
        ]
    )


def integration(df_stocks: pl.DataFrame, df_topix: pl.DataFrame):
    words = ["Long", "Short", "Control"]
    for word in words:
        print(word)
        df = df_stocks.filter(pl.col("long_or_short_or_control") == word)
        df = (
            df.group_by("date").agg(pl.col("day1_with_stop_loss_38").sum()).sort("date")
        )

        df_positive_window = df_topix.filter(pl.col("open_diff") > 1.0)
        df_positive_integrated = df_positive_window.join(df, on="date", how="inner")
        print("positive window")
        print(df_positive_integrated.get_column("day1_with_stop_loss_38").describe())
        df_negative_window = df_topix.filter(pl.col("open_diff") < 1.0)
        df_negative_integrated = df_negative_window.join(df, on="date", how="inner")
        print("negative window")
        print(df_negative_integrated.get_column("day1_with_stop_loss_38").describe())
        # if word == "Long":
        #     df_topix_2 = df_topix.filter(pl.col("open_diff") > 1.0)
        # elif word == "Short":
        #     df_topix_2 = df_topix.filter(pl.col("open_diff") < 1.0)
        # else:
        #     df_topix_2 = df_topix

        # df_integrated = df.join(df_topix_2, on="date", how="inner")

        # print(df_integrated.tail(8))
        # print(df_integrated.get_column("day1_with_stop_loss_38").describe())
    return
