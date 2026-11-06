package
{
    import flash.external.ExternalInterface;

    // The exe's calls of srp's own settings that it keeps in user.cfg (SrpCheckOption,
    // SrpMouseVertical): ExternalInterface.call(name[, value]). The exe answers before the call
    // returns (root.srp_check_option, root.srp_slider_option), so an answer arrives in the middle
    // of whatever asked. Outside the game there is no exe: `standIn` takes the calls instead when
    // set (scripts/hud-options' harness models the exe's patches with it); otherwise nothing happens.
    public class SrpExe
    {
        public static var standIn:Function = null;

        // A line of srp_patches.log (patch 83's srp_log: `srp: menu: <text>`), and the trace (Ruffle's log).
        public static function log(text:String):void
        {
            trace("SRP LOG " + text);
            call("srp_log", text);
        }

        public static function call(name:String, value:String):void
        {
            if (standIn != null)
            {
                standIn(name, value);
                return;
            }
            try
            {
                if (value == null)
                {
                    ExternalInterface.call(name);
                }
                else
                {
                    ExternalInterface.call(name, value);
                }
            }
            catch (error:Error)
            {
            }
        }
    }
}
