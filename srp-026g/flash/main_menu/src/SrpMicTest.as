package
{
    import flash.display.DisplayObject;
    import flash.display.MovieClip;
    import scaleform.clik.events.ButtonEvent;

    // The Sound tab's "Test microphone" button (exe patch 66, docs/notes/voice-chat.md, "Microphone
    // test"; the owner, 2026-11-14: "you press button, it records for 10 seconds, then plays it back"),
    // the last of the tab's voice controls, under the Microphone sensitivity slider and its PTT
    // checkbox: the tab's own button (SetingsButton, the Controls and Graphics tabs' Default, as the
    // Vulkan options' is). A press tells the exe the two voice sliders as they stand, accepted or
    // not: ExternalInterface.call("srp_mic_test", "<volume> <sensitivity>"). The voice DLL then records,
    // plays back and is idle again, or stops if it was doing so; the Settings window closed tells the
    // exe "stop" (the exe itself stops a test at a tab's change and when the main menu closes: the
    // button is not seen then). The button shows only what the exe
    // says: root.srp_mic_test(text), its whole text ("Test microphone", "Recording 7", "Playing 4",
    // "No microphone"), in answer to the tab's ask (the call with no value) and whenever the test's
    // state or its second changes. It keeps no state and no clock of its own. Not a setting: nothing
    // for Apply or Cancel. A script-added child of the tab content, shown on the Sound frame once the
    // exe has answered (without the voice DLL it never does).
    public class SrpMicTest
    {
        public static const NAME:String = "srp_mic_test";
        private static const SOUND_FRAME:int = 4;
        // The timeline's names of the two voice sliders ("Voice chat": its "c" is the Cyrillic one).
        private static const VOLUME:String = "_sound_сhat";
        private static const SENSITIVITY:String = "_mic_sens";
        // The call's value when the Settings window closes.
        private static const STOP:String = "stop";
        // The button's width whatever it says: the Settings' lists' (the minimap position's box, the
        // Monitor list's open list), which every dictionary's texts fit (scripts/mic-test checks).
        private static const WIDTH:Number = 184;

        public var button:SetingsButton;
        private var content:MovieClip;
        // The exe has answered: the button has its text.
        private var answered:Boolean = false;

        public function SrpMicTest(content:MovieClip)
        {
            this.content = content;
            button = new SetingsButton();
            button.name = "_" + NAME;
            button.autoSize = "none";
            button.focusable = true;
            button.toggle = false;
            button.visible = false;
            button.addEventListener(ButtonEvent.CLICK, onPress);
            content.addChild(button);
        }

        // On every bind of a tab (create_content): on top of the tab, the exe asked on the Sound frame.
        public function place():void
        {
            content.setChildIndex(button, content.numChildren - 1);
            show();
            if (content.currentFrame == SOUND_FRAME)
            {
                SrpExe.call(NAME, null);
            }
        }

        // After the tab's settings are bound: under the voice column's last control (the PTT checkbox)
        // as the sensitivity slider, by its label's top, is under the column's first (Use microphone),
        // from the column's left edge.
        public function layout():void
        {
            var sensitivity:DisplayObject = slider(SENSITIVITY) as DisplayObject;
            var first:DisplayObject = content.currentFrame == SOUND_FRAME ? content._use_mik : null;
            var last:DisplayObject = content.currentFrame == SOUND_FRAME ? content._ptt : null;
            if (sensitivity == null || first == null || last == null)
            {
                return;
            }
            button.x = sensitivity.x;
            button.y = Math.round(last.y + sensitivity.getBounds(content).top - first.y);
            button.width = WIDTH;
        }

        // The Settings window closes (Settings.hide): a test does not go on unseen.
        public function stop():void
        {
            if (answered)
            {
                SrpExe.call(NAME, STOP);
            }
        }

        // The exe's word (root.srp_mic_test): the button's text now.
        public function answer(text:String):void
        {
            button.label = text;
            answered = true;
            show();
        }

        private function onPress(event:ButtonEvent):void
        {
            var sensitivity:Object = slider(SENSITIVITY);
            var volume:Object = slider(VOLUME);
            if (sensitivity != null && volume != null)
            {
                SrpExe.call(NAME, Math.round(volume.selectedIndex) + " " + Math.round(sensitivity.selectedIndex));
            }
        }

        private function slider(name:String):Object
        {
            return content.currentFrame == SOUND_FRAME ? content[name] : null;
        }

        private function show():void
        {
            button.visible = content.currentFrame == SOUND_FRAME && answered;
            // Scaleform leaves a script-created control fully transparent.
            button.alpha = 1;
        }
    }
}
