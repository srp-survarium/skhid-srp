package
{
    import flash.display.MovieClip;
    import flash.external.ExternalInterface;
    import flash.text.TextField;
    import flash.text.TextFormat;
    import scaleform.clik.data.DataProvider;
    import scaleform.clik.events.IndexEvent;

    // The gameplay tab's Lobby choice (exe patch 81,
    // docs/notes/lobby-scenes.md): the game's arrow selector (the video
    // options' `Label:  < value >` stepper, a row of its own) over every
    // lobby scene, and the note. Settings owns it and counts a pick as a pending change of the
    // tab: Apply saves it (save), Cancel or leaving without Apply puts the
    // saved one back (revert). The exe keeps the choice in user.cfg and
    // answers ExternalInterface.call("srp_lobby_scene"[, id]) with
    // root.srp_lobby_scene(label, note, saved, shown, id, name, ...), which
    // Settings passes to answer. The note shows while the saved lobby is not
    // the one shown (the scene loads once, at the game's start). The controls
    // are script-added children of the tab content, which survive frame
    // changes: show keeps them on the gameplay frame only.
    public class SrpLobbyScene
    {
        private static const GAMEPLAY_FRAME:int = 1;
        // The bottom panel's right half (the Language choice has the left,
        // SrpLanguage): the selector centred on it, on the Language list's
        // row (the stepper keeps its own width: OptionStepperCyclic does not
        // lay itself out again), the note under it as the Language's.
        private static const CENTER_X:Number = 457;
        private static const STEPPER_Y:Number = 315;
        private static const NOTE_Y:Number = 348;
        private static const TEXT_WIDTH:Number = 300;

        public var stepper:OptionStepperCyclic;
        private var content:MovieClip;
        private var note:TextField;
        // The lobbies' ids in the selector's order (none until the exe
        // answers), the saved and the shown one.
        private var ids:Array = [];
        private var saved:String = "";
        private var shown:String = "";
        // The lobby the selector is at: the one revert puts it on, then each
        // arrow's. The stepper takes a new index only at its next draw (and
        // tells it then, INDEX_CHANGE), so its own selectedIndex lags a revert.
        private var index:int = -1;

        public function SrpLobbyScene(content:MovieClip)
        {
            this.content = content;
            stepper = new OptionStepperCyclic();
            stepper.name = "_srp_lobby_scene";
            stepper.focusable = true;
            // before Settings' listener: pending is this one's by then
            stepper.addEventListener(IndexEvent.INDEX_CHANGE, stepped);
            stepper.x = Math.round(CENTER_X - stepper.width / 2);
            stepper.y = STEPPER_Y;
            // Scaleform leaves a script-created control fully transparent.
            stepper.alpha = 1;
            content.addChild(stepper);
            note = text(SrpLanguage.textFormat(content), NOTE_Y, 40);
            note.name = "_srp_lobby_scene_note";
            note.wordWrap = true;
            note.multiline = true;
            note.alpha = 0.7;
            content.addChild(note);
            show();
        }

        // On every bind of a tab (create_content): shown on the gameplay
        // frame, where the exe is asked for the lobbies.
        public function place():void
        {
            show();
            if (content.currentFrame == GAMEPLAY_FRAME)
            {
                ask(null);
            }
        }

        // The exe's answer: label, note, saved, shown, then id and name
        // pairs. The saved lobby is selected.
        public function answer(values:Array):void
        {
            if (values.length < 6)
            {
                return;
            }
            stepper.label = values[0];
            note.text = values[1];
            saved = values[2];
            shown = values[3];
            ids = [];
            var names:Array = [];
            for (var i:int = 4; i + 1 < values.length; i += 2)
            {
                ids.push(values[i]);
                names.push(values[i + 1]);
            }
            // The exe answers a lobby it knows (any other saved id is its
            // classic); a list without the saved one is taken the same way.
            if (ids.indexOf(saved) < 0)
            {
                saved = ids[0];
            }
            stepper.dataProvider = new DataProvider(names);
            revert();
            show();
        }

        // The selected lobby is not the saved one: a change for Apply.
        public function get pending():Boolean
        {
            return index >= 0 && index < ids.length && ids[index] != saved;
        }

        // Apply: the selected lobby becomes the saved one (the exe writes it
        // to user.cfg and answers).
        public function save():void
        {
            if (!pending)
            {
                return;
            }
            saved = ids[index];
            show();
            ask(saved);
        }

        // Cancel: the saved lobby is selected again.
        public function revert():void
        {
            index = Math.max(0, ids.indexOf(saved));
            stepper.selectedIndex = index;
        }

        private function stepped(event:IndexEvent):void
        {
            index = event.index;
        }

        private function show():void
        {
            stepper.visible = content.currentFrame == GAMEPLAY_FRAME && ids.length > 0;
            note.visible = stepper.visible && saved != shown;
        }

        private static function text(format:TextFormat, y:Number, height:Number):TextField
        {
            var field:TextField = new TextField();
            field.embedFonts = true;
            field.selectable = false;
            field.mouseEnabled = false;
            field.defaultTextFormat = format;
            field.x = CENTER_X - TEXT_WIDTH / 2;
            field.y = y;
            field.width = TEXT_WIDTH;
            field.height = height;
            return field;
        }

        // The exe answers at once (root.srp_lobby_scene). Outside the game (a
        // test player) there is no exe to call: nothing happens.
        private static function ask(id:String):void
        {
            try
            {
                if (id == null)
                {
                    ExternalInterface.call("srp_lobby_scene");
                }
                else
                {
                    ExternalInterface.call("srp_lobby_scene", id);
                }
            }
            catch (error:Error)
            {
            }
        }
    }
}
