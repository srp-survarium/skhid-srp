package
{
    import flash.display.MovieClip;
    import flash.external.ExternalInterface;

    // A gameplay tab checkbox that the exe keeps in user.cfg as the console
    // variable of its name (exe patch 83's "Hide hit markers" and "Hide
    // scores", docs/notes/hide-hit-markers-scores.md). Settings owns it and
    // counts a toggle as a pending change of the tab: Apply saves it (save),
    // Cancel or leaving without Apply puts the saved state back (revert). The
    // exe answers ExternalInterface.call(name[, "1" or "0"]) with
    // root.srp_check_option(name, label, saved), saved "1" or "0", which
    // Settings passes to the option of that name (answer). The checkbox is a
    // script-added child of the tab content, which survives frame changes:
    // place shows it on the gameplay frame only, once the exe has answered.
    public class SrpCheckOption
    {
        private static const GAMEPLAY_FRAME:int = 1;

        public var box:CheckBoxSkinned;
        // The exe's name of the setting (its call and its user.cfg variable).
        public var name:String;
        private var content:MovieClip;
        // The saved state, "1" or "0"; "" until the exe answers.
        private var saved:String = "";

        // At (x, y) of the tab content, at the tab's checkbox size (a width
        // would scale the box: CLIK counter-scales the label only).
        public function SrpCheckOption(content:MovieClip, name:String, x:Number, y:Number)
        {
            this.content = content;
            this.name = name;
            box = new CheckBoxSkinned();
            box.name = "_" + name;
            box.autoSize = "none";
            box.enableInitCallback = false;
            box.focusable = true;
            box.selected = false;
            box.x = x;
            box.y = y;
            // Scaleform leaves a script-created control fully transparent.
            box.alpha = 1;
            content.addChild(box);
            show();
        }

        // On every bind of a tab (create_content): on top of the tab (a tab
        // change puts the tab's own timeline children back over script-added
        // ones, the bottom panel's translucent rect among them, which would
        // dim and cover the box), shown on the gameplay frame, where the exe
        // is asked for the state.
        public function place():void
        {
            content.setChildIndex(box, content.numChildren - 1);
            show();
            if (content.currentFrame == GAMEPLAY_FRAME)
            {
                ask(null);
            }
        }

        // The exe's answer after the name: the label and the saved state,
        // which is checked.
        public function answer(values:Array):void
        {
            if (values.length < 2 || (values[1] != "1" && values[1] != "0"))
            {
                return;
            }
            box.label = values[0];
            saved = values[1];
            revert();
            show();
        }

        // The checkbox is not the saved state: a change for Apply.
        public function get pending():Boolean
        {
            return saved != "" && state(box.selected) != saved;
        }

        // Apply: the checkbox becomes the saved state (the exe writes it to
        // user.cfg and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = state(box.selected);
            ask(saved);
        }

        // Cancel: the saved state is checked again.
        public function revert():void
        {
            box.selected = saved == "1";
        }

        private function show():void
        {
            box.visible = content.currentFrame == GAMEPLAY_FRAME && saved != "";
        }

        private static function state(selected:Boolean):String
        {
            return selected ? "1" : "0";
        }

        // The exe answers at once (root.srp_check_option). Outside the game (a
        // test player) there is no exe to call: nothing happens.
        private function ask(state:String):void
        {
            try
            {
                if (state == null)
                {
                    ExternalInterface.call(name);
                }
                else
                {
                    ExternalInterface.call(name, state);
                }
            }
            catch (error:Error)
            {
            }
        }
    }
}
