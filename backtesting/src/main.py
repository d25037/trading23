import pandas_analysis as my_pandas
import polars_analysis as my_polars


def main():
    df_pl = my_polars.read_json_as_pd("jquants_backtest.json")
    print(df_pl.head())

    (
        long_pl,
        short_pl,
        control_pl,
        without_control_pl,
    ) = my_polars.filter_by_long_short_control(df_pl)

    long_filter_by_diff_pl = my_polars.filter_by_standardized_diff(
        "Long", long_pl, 0.085, 0.125
    )

    my_polars.group_by_diff(long_pl)

    short_filter_by_diff_pl = my_polars.filter_by_standardized_diff(
        "Short", short_pl, 0.075, 0.125
    )

    my_polars.group_by_diff(short_pl)

    # control_filter_by_diff_pl = my_polars.filter_by_standardized_diff(
    #     "Control", control_pl, 0.085, 0.125
    # )
    # my_pandas.analyze_settlement_order(long_filter_by_diff)

    # entry_date = my_pandas.daily_long_short(without_control)
    # my_pandas.backtest(without_control, entry_date)

    df_coin_pl = my_polars.read_json_as_pd("gmo_coin_backtest.json")
    print(df_coin_pl.head())

    (
        long_coin_pl,
        short_coin_pl,
        control_coin_pl,
        without_control_coin_pl,
    ) = my_polars.filter_by_long_short_control(df_coin_pl)

    long_filter_by_diff_pl = my_polars.filter_by_standardized_diff(
        "Long", long_coin_pl, 0.11, 0.13
    )
    my_polars.group_by_diff(long_coin_pl)

    short_filter_by_diff_pl = my_polars.filter_by_standardized_diff(
        "Short", short_coin_pl, 0.11, 0.13
    )

    my_polars.group_by_diff(short_coin_pl)

    # without_control_filter_by_diff_pl = my_polars.filter_by_standardized_diff(
    #     "w/o control", without_control_coin_pl, 0.11, 0.13
    # )

    # control_filter_by_diff_pl = my_polars.filter_by_standardized_diff(
    #     "control", control_coin_pl, 0.11, 0.13
    # )

    # my_polars.plot_cum_sum(long_filter_by_diff_pl["strong"], "day20_with_stop_loss_2")

    # # dfb.backtest(long_filter_by_diff["strong"])
    return


if __name__ == "__main__":
    main()
