import matplotlib.pyplot as plt
import numpy as np
import polars as pl


def load_trade_data(file_path):
    # 取引データを読み込む処理
    df = pl.read_csv(file_path)

    # カスタム関数を定義
    def custom_cast(value):
        try:
            return int(value)
        except ValueError:
            return 0

    df = df.with_columns(pl.col("受渡金額/決済損益").apply(custom_cast).alias("収支"))

    return df


def display_trade_balance(df):
    # 取引収支を表示する処理
    # print(df.group_by("決済日").agg(pl.col("収支")).sum().sort("決済日"))
    with pl.Config(tbl_rows=300):
        print(
            df.select(
                [
                    "決済日",
                    "銘柄コード",
                    "銘柄",
                    "買/売建",
                    "決済数量",
                    "建単価",
                    "収支",
                ]
            ).tail(300)
        )
    df = (
        df.group_by("決済日")
        .agg(
            pl.col("収支")
            .apply(lambda x: sum(x) if x is not None else None)
            .alias("収支")
        )
        .sort("決済日")
    )

    with pl.Config(tbl_rows=100):
        print(df)
    return df


def visualize_trade_balance(df):
    cum_summed_series = df.get_column("収支").cum_sum()
    x = df.get_column("決済日").to_numpy()

    plt.plot(x, cum_summed_series.to_numpy())

    selected_indices = np.linspace(0, len(x) - 1, 5, dtype=int)
    selected_labels = [str(x[i]) for i in selected_indices]  # Convert labels to strings
    plt.xticks(selected_indices, selected_labels)

    plt.show()
