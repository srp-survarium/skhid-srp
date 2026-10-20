package
{
    import survarium.controls.Message;

    // Every "Get Gold" of the lobby opened the online store, which is gone:
    // it opens srp's exchange window (SrpExchange) instead, to buy gold with
    // silver. A window behind it (the premium window, for one) stays
    // visible under it and has the focus again when it closes.
    public class SrpBuyGold
    {
        // The dialogs call it with their data; the premium window's gold
        // button with its click: neither is needed.
        public static function open(... unused):void
        {
            // Shown at once over a window still open (without the third
            // argument it would wait for that window to close), which stays
            // visible under it (the fifth), as the premium windows' own
            // dialogs do.
            Message.showMessage("exchange", {"label": "", "srpBuyGold": true}, true, 0, true);
        }
    }
}
