package
{
    import flash.display.DisplayObject;
    import flash.display.Loader;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.events.UncaughtErrorEvent;
    import flash.geom.Rectangle;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.text.Font;
    import flash.utils.getQualifiedClassName;

    // The Settings' sound tab with its "Test microphone" button (exe patch 66, docs/notes/voice-chat.md,
    // "Microphone test") in the rebuilt main menu, the exe stood in for (SrpExe.standIn): no button
    // until the exe answers the tab's ask; its place under the voice controls, the last of them; a press
    // sends the two voice sliders as they stand and is no change for Apply; the Settings window closed
    // says stop; the button shows the exe's text, every dictionary's on one line with the font's glyphs;
    // hidden on the other tabs and asked again on the way back. And its "Microphone" list (voice-chat.md,
    // "Microphone"): none until the exe answers; Default first, then the devices by name; its place between
    // the slider and the button; a pick pending for Apply, Cancel putting the saved one back, Apply
    // sending its id ("" the default); an unplugged device shown as Default; the list asked again as it
    // is pressed open (a pick not applied kept); every dictionary's two texts fitting. Prints "MICTEST ..." lines and "MICTEST RESULT pass" or "MICTEST RESULT
    // fail: ...", then shows each state of the button for check.sh's screenshots ("MICTEST SHOT
    // <name>") and ends with "MICTEST END". No game, no voice DLL, no audio device.
    public class MicTestHarness extends Sprite
    {
        private static const NAME:String = "srp_mic_test";
        // check.sh's texts, a row per dictionary: the button's five (idle, recording, playing, no
        // microphone, no playback device), then the tab's stock ones.
        private static const IDLE:int = 0, RECORDING:int = 1, PLAYING:int = 2, NO_MICROPHONE:int = 3, NO_PLAYBACK:int = 4;
        private static const STOCK:int = 5;
        // ... then the Microphone list's label and its default's text.
        private static const MIC_LABEL:int = STOCK + 10, MIC_DEFAULT:int = STOCK + 11;
        private static const MICROPHONE:String = "srp_microphone";
        private static const VOLUME:String = "_sound_сhat";

        private var loader:Loader = new Loader();
        private var frames:int = 0;
        private var texts:Array;
        // What the stand-in exe was called with for the test: each call's value (null: the ask).
        private var calls:Array = [];
        // Whether the stand-in exe has the voice DLL (it answers the ask).
        private var dll:Boolean = false;
        // The button's text as the stand-in exe words the test's state.
        private var word:String = "";
        // The stand-in exe's capture devices ([id, name], Windows' order), its saved pick ("": the
        // default), the dictionary it answers in, and each call of the list (null: an ask).
        private var devices:Array = [["{0.0.1.00000000}.{a1}", "Microphone (Realtek(R) Audio)"],
            ["{0.0.1.00000000}.{b2}", "Headset Microphone (SteelSeries Arctis 7 Chat)"],
            ["{0.0.1.00000000}.{c3}", "Микрофон (USB Audio Device)"]];
        private var saved:String = "";
        private var dictionary:Array;
        private var microphoneCalls:Array = [];

        public function MicTestHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("MICTEST RESULT fail: " + e);
            });
            loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,
                function(e:UncaughtErrorEvent):void { e.preventDefault(); });
            addChild(loader);
            loader.load(new URLRequest("main_menu.swf"), new LoaderContext(false, new ApplicationDomain()));
        }

        private function loaded(e:Event):void
        {
            addEventListener(Event.ENTER_FRAME, tick);
        }

        private function require(value:Boolean, message:String):void
        {
            if (!value) throw new Error(message);
        }

        private function get menu():* { return loader.content; }
        private function get settings():* { return menu.settings_window; }
        private function get content():* { return settings.settings_content; }
        private function get button():* { return content.getChildByName("_" + NAME); }
        private function get list():* { return content.getChildByName("_" + MICROPHONE); }
        private function get listLabel():* { return content.getChildByName("_srp_microphone_label"); }

        private function click(target:*):void
        {
            var buttonEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
            target.dispatchEvent(new buttonEvent("buttonClick"));
        }

        // The exe: patch 66's answer to the Settings' ask of the test; a press is only taken.
        private function exe(name:String, value:String):void
        {
            if (name == MICROPHONE)
            {
                microphoneCalls.push(value);
                if (value != null) saved = value;
                if (dll) answerMicrophones();
                return;
            }
            if (name != NAME) return;
            calls.push(value);
            if (value == null && dll) menu.srp_mic_test(word);
        }

        // The exe: patch 66's answer to the list (root.srp_microphone): its label and the default's text in
        // the dictionary it answers in, the saved device while it is listed (else "", the default), then
        // each device's id and name.
        private function answerMicrophones():void
        {
            var values:Array = [dictionary[MIC_LABEL], dictionary[MIC_DEFAULT], ""];
            for each (var device:Array in devices)
            {
                values.push(device[0], device[1]);
                if (device[0] == saved) values[2] = saved;
            }
            menu.srp_microphone.apply(null, values);
        }

        // The Settings' own texts as the exe gives them once (which puts the tab bar on its first tab).
        private function labels(row:Array):void
        {
            menu.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                {name: "video_options_type", label: "Graphics"}, {name: "sound_options_type", label: row[STOCK + 9]},
                {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"},
                {name: "s_sound_volume", label: row[STOCK + 7]}, {name: "s_voice_chat", label: row[STOCK + 8]}]);
            menu.set_settings(0, []);
        }

        // The sound tab as the exe binds it: the stock labels of dictionary `row`, the sliders 0..100
        // in steps of 1 (the float options' steps), the two checkboxes.
        private function bind(row:Array, volume:int, sensitivity:int):void
        {
            var slider:Object = {minimum: 0, maximum: 100, snapInterval: 1};
            var list:Array = [];
            for (var i:int = 0; i < 7; ++i)
            {
                var setting:Object = {label: row[STOCK + i], setter: function(value:*):* { return value; }};
                if (i < 4 || i == 5) setting.dataProvider = slider;
                list.push(setting);
            }
            menu.set_settings(3, list);
            menu.set_values(3, [{id: 0, value: 100}, {id: 1, value: 100}, {id: 2, value: 40}, {id: 3, value: volume},
                {id: 4, value: 1}, {id: 5, value: sensitivity}, {id: 6, value: 0}]);
        }

        // The button's text is the exe's `text`, drawn whole on its one line.
        private function shows(text:String, when:String):void
        {
            menu.srp_mic_test(text);
            button.validateNow();
            require(button.label == text && button.textField.text == text, when + ": the button says " + button.textField.text);
            require(button.textField.textWidth + 4 <= button.textField.width * button.scaleX + 0.01,
                when + ": " + text + " does not fit (" + button.textField.textWidth + " of " + button.textField.width * button.scaleX + ")");
            require(button.textField.numLines == 1, when + ": " + text + " is not one line");
        }

        private function numbered(text:String, seconds:int):String
        {
            return text.split("%1$s").join(String(seconds));
        }

        private function check():void
        {
            var c:* = content;
            var b:* = button;
            require(b != null && b.visible && b.alpha == 1, "the button shows on the sound tab once the exe has answered");
            require(calls.length == 2 && calls[0] == null && calls[1] == null, "each bind of the sound tab asks the exe once: " + calls);
            require(b.label == texts[0][IDLE], "its text is the exe's: " + b.label);
            require(getQualifiedClassName(b) == "SetingsButton", "it is the tab's own button");
            // the stock PTT checkbox hidden (the owner, 2026-11-14: "let's remove the button then")
            require(c._ptt != null && !c._ptt.visible, "the PTT checkbox is hidden");
            // under the voice column's controls (the sensitivity slider, the Microphone list: checkMicrophone),
            // from the column's left edge, clear of every other control
            var slider:Rectangle = c._mic_sens.getBounds(c);
            var at:Rectangle = b.getBounds(c);
            require(Math.abs(at.left - slider.left) <= 1 && Math.abs(at.left - c._use_mik.x) <= 1,
                "its left edge is the voice column's (" + at.left + ", " + slider.left + ")");
            require(at.top >= slider.bottom, "it is under the sensitivity slider (" + at.top + ", " + slider.bottom + ")");
            require(at.right <= slider.right, "it ends within the column (" + at.right + ", " + slider.right + ")");
            for (var i:int = 0; i < c.numChildren; ++i)
            {
                var other:DisplayObject = c.getChildAt(i);
                if (other == b || !other.visible) continue;
                require(!at.intersects(other.getBounds(c)), "it lies over " + other.name + " (" + at + ", " + other.getBounds(c) + ")");
            }
            trace("MICTEST button " + Math.round(at.width) + "x" + Math.round(at.height) + " at " + Math.round(at.left) + "," + Math.round(at.top)
                + ", the sensitivity slider ends at " + Math.round(slider.bottom));

            // a press: the two voice sliders as they stand; no change for Apply
            require(!settings.btn_accept.enabled, "Apply is off before a press");
            calls = [];
            click(b);
            require(calls.length == 1 && calls[0] == "100 50", "a press sends the sliders: " + calls);
            require(!settings.btn_accept.enabled, "a press is no change of the tab");
            // the Settings window closed (its Exit, its cross, Esc): the exe is told to stop
            menu.show_settings(false);
            require(!settings.visible && calls.length == 2 && calls[1] == "stop", "the window closed says stop: " + calls);
            menu.show_settings(true);
            require(settings.visible && b.visible, "the window shown again has its button");
            calls = [calls[0]];
            // moved and not accepted: the press sends what is on the screen
            c[VOLUME].selectedIndex = 35;
            c._mic_sens.selectedIndex = 80;
            click(b);
            require(calls.length == 2 && calls[1] == "35 80", "a press sends the sliders as they stand: " + calls);
            // the button shows what the exe says, whatever it is, and keeps it until the next word
            shows(numbered(texts[0][RECORDING], 10), "english");
            click(b);
            require(calls.length == 3 && calls[2] == "35 80" && b.label == numbered(texts[0][RECORDING], 10),
                "a press while it records only tells the exe: " + calls + ", " + b.label);

            // every dictionary's five texts, the counted ones at their longest
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            var font:Font = null;
            for each (var fontClass:String in ["CalibriBold", "FontTB", "Calibri", "FontT"])
            {
                if (!domain.hasDefinition(fontClass)) continue;
                var candidate:Font = new (domain.getDefinition(fontClass) as Class)() as Font;
                if (candidate.fontName == b.textField.getTextFormat().font
                    && candidate.fontStyle == (b.textField.getTextFormat().bold ? "bold" : "regular")) font = candidate;
            }
            require(font != null, "the button's font is a class of the menu: " + b.textField.getTextFormat().font);
            var widest:Number = 0, longest:String = "";
            for (var row:int = 0; row < texts.length; ++row)
            {
                for (var state:int = IDLE; state <= NO_PLAYBACK; ++state)
                {
                    var text:String = numbered(texts[row][state], 10);
                    shows(text, "dictionary " + row);
                    require(font.hasGlyphs(text), font.fontName + " lacks a letter of " + text);
                    if (b.textField.textWidth > widest)
                    {
                        widest = b.textField.textWidth;
                        longest = text;
                    }
                }
            }
            trace("MICTEST " + texts.length + " dictionaries' texts fit the button in " + font.fontName + " " + font.fontStyle
                + "; the widest " + longest + ", " + Math.round(widest) + " of " + Math.round(b.textField.width * b.scaleX) + " px");

            // another tab: no button; back: asked again, the exe's text of then
            calls = [];
            word = numbered(texts[0][PLAYING], 4);
            settings.selectIndex(0);
            require(!b.visible, "no button on the gameplay tab");
            require(calls.length == 0, "the gameplay tab does not ask: " + calls);
            settings.selectIndex(3);
            require(b.visible && calls.length == 1 && calls[0] == null && b.label == word, "back on the sound tab it asks: " + calls + ", " + b.label);
            require(!content._ptt.visible, "back on the sound tab the PTT checkbox is still hidden");
            word = texts[0][IDLE];
            menu.srp_mic_test(word);
        }

        // A player's pick: the list dispatches its change only while open.
        private function pick(m:*, index:int):void
        {
            var listEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ListEvent") as Class;
            m.selectedIndex = index;
            m.dispatchEvent(new listEvent("listIndexChange", true, false, index));
        }

        // The pick shown in the closed list fits its box (or not: how wide it is).
        private function fits(m:*):Boolean
        {
            m.validateNow();
            return m.textField.textWidth + 4 <= m.textField.width * m.scaleX + 0.01;
        }

        // The Microphone list (voice-chat.md, "Microphone"), with the stand-in exe's three devices.
        private function checkMicrophone():void
        {
            var c:* = content;
            var m:* = list;
            var l:* = listLabel;
            var b:* = button;
            require(m != null && m.visible && m.alpha == 1 && l.visible, "the Microphone list shows once the exe has answered");
            require(getQualifiedClassName(m) == "DropdownMenuSkinned", "it is the movie's own list: " + getQualifiedClassName(m));
            require(l.text == texts[0][MIC_LABEL], "its label is the exe's: " + l.text);
            var names:Array = [];
            for (var n:int = 0; n < m.dataProvider.length; ++n) names.push(m.dataProvider[n]);
            require(names.join("|") == [texts[0][MIC_DEFAULT], devices[0][1], devices[1][1], devices[2][1]].join("|"),
                "Default first, then the devices by their names: " + names);
            require(m.selectedIndex == 0 && !settings.btn_accept.enabled, "none saved: Default, nothing pending");

            // the column: the slider, the list's label in the PTT checkbox's row, the list, the button
            m.validateNow();
            var slider:Rectangle = c._mic_sens.getBounds(c);
            var label:Rectangle = new Rectangle(l.x, l.y, l.textWidth + 4, l.textHeight + 4);
            var box:Rectangle = m.getBounds(c);
            var at:Rectangle = b.getBounds(c);
            require(label.top >= slider.bottom && box.top >= label.bottom - 1 && at.top >= box.bottom,
                "top to bottom: the slider, the label, the list, the button (" + [slider.bottom, label.top, label.bottom, box.top, box.bottom, at.top] + ")");
            require(Math.abs(box.left - at.left) <= 1 && Math.abs(l.x - c._mic_sens.textField.getBounds(c).left) <= 1,
                "the list and the button at the column's edge, the label at the slider's label's (" + [box.left, at.left, l.x] + ")");
            require(box.right <= slider.right + 1, "the list ends within the column (" + box.right + ", " + slider.right + ")");
            for (var i:int = 0; i < c.numChildren; ++i)
            {
                var other:DisplayObject = c.getChildAt(i);
                if (other == m || other == l || !other.visible) continue;
                require(!box.intersects(other.getBounds(c)), "the list lies over " + other.name + " (" + box + ", " + other.getBounds(c) + ")");
                require(!label.intersects(other.getBounds(c)), "its label lies over " + other.name + " (" + label + ", " + other.getBounds(c) + ")");
            }
            trace("MICTEST list " + Math.round(box.width) + "x" + Math.round(box.height) + " at " + Math.round(box.left) + "," + Math.round(box.top)
                + ", its label at " + Math.round(label.top) + ", the button at " + Math.round(at.top));

            // a pick is a change for Apply; Cancel puts the saved one back
            pick(m, 2);
            require(settings.btn_accept.enabled, "a pick is a change for Apply");
            click(settings.btn_cancel);
            require(m.selectedIndex == 0 && !settings.btn_accept.enabled, "Cancel puts the saved one back: " + m.selectedIndex);
            // a pick applied: the exe is given its id, and answers with it shown
            microphoneCalls = [];
            pick(m, 2);
            click(settings.btn_accept);
            require(microphoneCalls.length == 1 && microphoneCalls[0] == devices[1][0] && saved == devices[1][0],
                "Apply sends the pick's id: " + microphoneCalls);
            require(m.selectedIndex == 2 && !settings.btn_accept.enabled, "applied, it is shown and nothing is pending");
            // unplugged: the list shows Default, nothing pending; the pick stays saved for when it is back
            var unplugged:Array = devices.splice(1, 1)[0];
            bind(texts[0], 100, 50);
            require(m.dataProvider.length == 3 && m.selectedIndex == 0 && !settings.btn_accept.enabled,
                "a saved device not there: Default shown, nothing pending (" + m.selectedIndex + ")");
            require(saved == unplugged[0], "and it stays saved");
            // plugged in again while the tab shows: pressed open, the list asks again and has it
            devices.splice(1, 0, unplugged);
            microphoneCalls = [];
            m.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
            require(microphoneCalls.length == 1 && microphoneCalls[0] == null, "pressed open, the list asks the exe again: " + microphoneCalls);
            require(m.dataProvider.length == 4 && m.selectedIndex == 2 && !settings.btn_accept.enabled, "the device back, shown as saved");
            // a pick not applied yet stays through an ask
            pick(m, 3);
            m.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
            require(m.selectedIndex == 3 && settings.btn_accept.enabled, "a pick not applied stays through an ask: " + m.selectedIndex);
            // open, a press closes it: no ask
            microphoneCalls = [];
            m.open();
            m.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN));
            m.close();
            require(microphoneCalls.length == 0, "a press on the open list does not ask: " + microphoneCalls);
            // Default applied: the empty id
            pick(m, 0);
            click(settings.btn_accept);
            require(microphoneCalls.length == 1 && microphoneCalls[0] == "" && saved == "", "Default applied: the empty id: " + microphoneCalls);

            // every dictionary's label and Default fit; how the devices' names fit the box
            for (var row:int = 0; row < texts.length; ++row)
            {
                dictionary = texts[row];
                answerMicrophones();
                require(l.text == texts[row][MIC_LABEL] && l.textWidth + 4 <= l.width && l.numLines == 1,
                    "dictionary " + row + ": the label " + l.text + " fits (" + l.textWidth + " of " + l.width + ")");
                require(m.selectedIndex == 0 && fits(m), "dictionary " + row + ": " + texts[row][MIC_DEFAULT] + " fits the list ("
                    + m.textField.textWidth + " of " + m.textField.width + ")");
            }
            dictionary = texts[0];
            answerMicrophones();
            var widths:Array = [];
            for (var d:int = 1; d < m.dataProvider.length; ++d)
            {
                m.selectedIndex = d;
                widths.push(m.dataProvider[d] + (fits(m) ? " fits, " : " does not fit, ") + Math.round(m.textField.textWidth) + " px");
            }
            m.selectedIndex = 0;
            trace("MICTEST the list's box has " + Math.round(m.textField.width * m.scaleX) + " px for its text: " + widths.join("; "));
        }

        private function tick(e:Event):void
        {
            ++frames;
            try
            {
                if (frames == 10)
                {
                    texts = JSON.parse(loaderInfo.parameters.texts) as Array;
                    dictionary = texts[0];
                    var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
                    Object(domain.getDefinition("SrpExe")).standIn = exe;
                    labels(texts[0]);
                    bind(texts[0], 100, 50);
                    menu.show_settings(true);
                    // no voice DLL: the exe does not answer, and there is no button
                    settings.selectIndex(3);
                }
                if (frames == 20)
                {
                    require(content.currentFrame == 4, "the sound tab is frame 4: " + content.currentFrame);
                    require(calls.length == 1 && calls[0] == null, "the sound tab asks the exe: " + calls);
                    require(button != null && !button.visible, "without the exe's answer there is no button");
                    require(microphoneCalls.length == 1 && microphoneCalls[0] == null, "the sound tab asks for the list: " + microphoneCalls);
                    require(list != null && !list.visible && !listLabel.visible, "without the exe's answer there is no Microphone list");
                    microphoneCalls = [];
                    // with it: the tab bound again (as the game does when its options are read)
                    dll = true;
                    word = texts[0][IDLE];
                    bind(texts[0], 100, 50);
                }
                if (frames == 30)
                {
                    check();
                    checkMicrophone();
                    trace("MICTEST RESULT pass");
                }
                // the states, for the screenshots (the first dictionary's), each held a second so check.sh's
                // screenshot is taken before the next
                if (frames == 50) trace("MICTEST SHOT idle");
                if (frames == 110) menu.srp_mic_test(numbered(texts[0][RECORDING], 7));
                if (frames == 130) trace("MICTEST SHOT recording");
                if (frames == 190) menu.srp_mic_test(numbered(texts[0][PLAYING], 4));
                if (frames == 210) trace("MICTEST SHOT playing");
                if (frames == 270) menu.srp_mic_test(texts[0][NO_MICROPHONE]);
                if (frames == 290) trace("MICTEST SHOT no-microphone");
                if (frames == 350) menu.srp_mic_test(texts[0][NO_PLAYBACK]);
                if (frames == 370) trace("MICTEST SHOT no-playback");
                if (frames == 430) menu.srp_mic_test(texts[0][IDLE]);
                if (frames == 440) list.open();
                if (frames == 450) trace("MICTEST SHOT microphones");
                if (frames == 510) list.close();
                if (frames == 520) trace("MICTEST END");
            }
            catch (error:Error)
            {
                trace("MICTEST RESULT fail: " + error.message);
                frames = 1000;
            }
        }
    }
}
