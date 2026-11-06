package
{
    import flash.display.DisplayObject;
    import flash.display.DisplayObjectContainer;
    import flash.utils.getQualifiedClassName;
    import flash.display.Loader;
    import flash.display.MovieClip;
    import flash.display.Sprite;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.events.UncaughtErrorEvent;
    import flash.geom.Rectangle;
    import flash.net.URLRequest;
    import flash.system.ApplicationDomain;
    import flash.system.LoaderContext;
    import flash.text.Font;
    import flash.text.TextField;

    // The Settings' gameplay tab with its "HUD:" group (exe patch 83,
    // docs/notes/hide-hit-markers-scores.md) in the rebuilt main menu: the
    // exe's answers (root.srp_check_option, root.srp_hud_texts), the tab's two
    // columns (Friends and Language; HUD: the crosshair list, Hide hit markers,
    // Hide player scores, Hide quests, Static minimap, the minimap position
    // list), their edges, spacing and fit in every dictionary; Hide quests as
    // the stock quest progress inverted; the crosshair list's Dynamic and
    // Static as the stock static crosshair, Hidden as patch 83's; each pending
    // until Apply, Cancel dropping it; the other tabs; the glyphs. Ends on the
    // gameplay tab, then with Hide player scores checked (Apply enabled), then
    // the crosshair list open, then the minimap position list open, for
    // check.sh's screenshots. Prints "SETTINGS ..." lines and "SETTINGS RESULT
    // pass" or "SETTINGS RESULT fail: ...". No game or server.
    public class SettingsHarness extends Sprite
    {
        private static const NAMES:Array = ["srp_hide_hit_markers", "srp_hide_scores", "srp_map_names"];
        // Each name's label in a row of check.sh's texts.
        private static const LABELS:Array = [0, 1, 13];

        private static const STOCK_LABELS:Array = ["Invitations from friends only", "Notify when friends are online",
            "Enable message censorship", "Accept private messages from friends only", "Hide chat in match", "Hide spam",
            "", "Static crosshair", "Fix minimap", "", "Show quest progress in match", "Transparency"];

        private var loader:Loader = new Loader();
        private var frames:int = 0;
        // The texts of check.sh (texts[n][14..16]: "Renderer", "DirectX 11", "Vulkan/DXVK").
        private var texts:Array;
        // The gameplay tab's settings and values as check() gave them.
        private var gameplay:Array;

        public function SettingsHarness()
        {
            loader.contentLoaderInfo.addEventListener(Event.INIT, loaded);
            loader.contentLoaderInfo.addEventListener("ioError", function(e:Event):void {
                trace("SETTINGS RESULT fail: " + e);
            });
            // The menu calls the exe (ExternalInterface), which a test player lacks.
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

        // A click of Apply or Cancel. Outside the game the exe calls after it
        // may throw; what the buttons did before them is what is checked.
        private function click(button:*):void
        {
            var buttonEvent:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.events.ButtonEvent") as Class;
            try
            {
                button.dispatchEvent(new buttonEvent("buttonClick"));
            }
            catch (error:Error)
            {
            }
        }

        private function tick(e:Event):void
        {
            if (++frames < 10) return;
            removeEventListener(Event.ENTER_FRAME, tick);
            try
            {
                check();
                trace("SETTINGS RESULT pass");
            }
            catch (error:Error)
            {
                trace("SETTINGS RESULT fail: " + error.message);
            }
            frames = 0;
            var settings:* = Object(loader.content).settings_window;
            addEventListener(Event.ENTER_FRAME, function(e:Event):void {
                ++frames;
                if (frames == 20)
                {
                    if (settings.btn_accept.enabled) trace("SETTINGS RESULT fail: Apply enabled before the screenshot");
                    trace("SETTINGS SHOT settings");
                }
                if (frames == 80)
                {
                    settings.settings_content.getChildByName("_srp_hide_scores").selected = true;
                    if (!settings.btn_accept.enabled) trace("SETTINGS RESULT fail: the toggle did not enable Apply");
                }
                if (frames == 100) trace("SETTINGS SHOT checked");
                if (frames == 110) settings.settings_content.getChildByName("_srp_crosshair").open();
                if (frames == 130) trace("SETTINGS SHOT open");
                if (frames == 200)
                {
                    settings.settings_content.getChildByName("_srp_crosshair").close();
                    settings.settings_content._setter_static_minimap_position.open();
                }
                if (frames == 230) trace("SETTINGS SHOT minimap");
                if (frames == 240)
                {
                    settings.settings_content._setter_static_minimap_position.close();
                    // the toggle of the checked shot dropped
                    click(settings.btn_cancel);
                }
                // the Graphics tab after the gameplay shots: its controls tell their changes a frame later
                if (frames == 250)
                {
                    try
                    {
                        checkGraphics(settings, settings.settings_content, gameplay[0], gameplay[1]);
                    }
                    catch (error:Error)
                    {
                        trace("SETTINGS RESULT fail: " + error.message);
                    }
                }
                if (frames == 270) graphicsTab(settings, "1");
                if (frames == 300) trace("SETTINGS SHOT graphics");
                // the Vulkan options' panel open (Vulkan picked), for its screenshot
                if (frames == 350) click(settings.settings_content.getChildByName("_srp_vulkan"));
                if (frames == 365) trace("SETTINGS SHOT vulkan");
                if (frames == 405) settings.closePanel();
                if (frames == 410)
                {
                    // Cancel, as a player would, what the earlier tabs left pending: checkGraphics leaves the
                    // Graphics tab in the frame it bound it (a player cannot), and that tab's stock lists and
                    // steppers tell their bound values a frame later, on the gameplay tab, as changes. With
                    // nothing pending Cancel is Exit, which would close Settings.
                    if (settings.btn_accept.enabled) click(settings.btn_cancel);
                    var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
                    var texts:Array = JSON.parse(loaderInfo.parameters.texts) as Array;
                    // the controls tab, after the gameplay tab's screenshots (its stock stepper takes a
                    // value of its own at a later visit, which a test player's tab bar cannot avoid)
                    try
                    {
                        checkControls(Object(loader.content), settings, texts, domain);
                        trace("SETTINGS controls pass");
                        checkBorders(Object(loader.content), settings, texts, domain);
                        trace("SETTINGS borders pass");
                        checkApplyCancel(Object(loader.content), settings, texts, domain);
                        trace("SETTINGS apply-cancel pass");
                    }
                    catch (error:Error)
                    {
                        trace("SETTINGS RESULT fail: " + error.message);
                    }
                    controls(Object(loader.content), settings, domain, 1.25);
                    Object(loader.content).srp_check_option("srp_mouse_vertical_locked", texts[0][17], "1");
                    Object(loader.content).srp_slider_option("srp_mouse_vertical", texts[0][18], "100");
                }
                if (frames == 440) trace("SETTINGS SHOT controls-off");
                if (frames == 450)
                {
                    settings.settings_content.getChildByName("_srp_mouse_vertical_locked").selected = false;
                    settings.settings_content.getChildByName("_srp_mouse_vertical").slider.value = 2.35;
                }
                if (frames == 480) trace("SETTINGS SHOT controls-on");
            });
        }

        // A visible control's or text's top and bottom in the tab content: a checkbox's box and drawn
        // label (its label's field runs on below), a text's drawn lines, a list's or slider's own height.
        private function span(item:*):Array
        {
            if (item is TextField)
                return [item.y, item.y + TextField(item).textHeight + 4];
            if (item.hasOwnProperty("textField") && item.textField != null && item.hasOwnProperty("selected"))
                return [item.y, item.y + Math.max(item.textField.y + item.textField.textHeight + 4, 26)];
            var bounds:Rectangle = item.getBounds(item.parent);
            return [bounds.top, bounds.bottom];
        }

        // Equal slider values (a slider keeps n x its snap, not the decimal).
        private static function close(a:Number, b:Number):Boolean
        {
            return Math.abs(a - b) < 1e-6;
        }

        // Equal places (x and y are kept in twips, 1/20 px).
        private static function near(a:Number, b:Number):Boolean
        {
            return Math.abs(a - b) < 0.1;
        }

        // The timeline's own places of the gameplay tab, from a new instance of its class (Settings has
        // moved the shown one's).
        private function stockOf(content:MovieClip):Object
        {
            var fresh:* = new (Object(content).constructor as Class)();
            fresh.gotoAndStop(1);
            return {gap: fresh.s_quest_propgress.y - fresh._setter_static_minimap_position.y,
                step: fresh._setter_message.y - fresh._setter_invite.y,
                leftFirst: fresh._setter_invite.y - fresh.s_friends.y,
                rightFirst: fresh._setter_static_cross.y - fresh.s_cross.y,
                dropdown: fresh._setter_static_minimap_position.y - fresh._setter_static_minimap.y,
                dropdownX: fresh._setter_static_minimap_position.x - fresh._setter_static_minimap.x,
                rightHeading: fresh.s_cross.x, rightControl: fresh._setter_static_cross.x};
        }

        // The gameplay tab's two columns: on the left "Friends:", then Language (its note under the list
        // while it shows); on the right "HUD:": the crosshair list, Hide hit markers, Hide player scores,
        // Hide quests (the stock quest progress checkbox), Static minimap (the stock fixed minimap
        // checkbox), the minimap position list. Not shown: the stock "Crosshair:" and "Quest progress:"
        // headings, the static crosshair checkbox, the quest progress transparency slider. Each heading
        // at its column's heading x, the controls at its controls' x (a list one further, as the
        // timeline has them), spaced as the tab's own; nothing overlapping in a column, everything above
        // the window's bottom; every dictionary's texts inside their column.
        private function checkLayout(content:MovieClip, boxes:Array, texts:Array, stock:Object, when:String):void
        {
            var c:* = content;
            var heading:TextField = content.getChildByName("_srp_check_group") as TextField;
            var menu:* = content.getChildByName("_srp_language");
            var cross:* = content.getChildByName("_srp_crosshair");
            var language:TextField = null, note:TextField = null;
            for (var i:int = 0; i < content.numChildren; ++i)
            {
                var t:TextField = content.getChildAt(i) as TextField;
                if (t != null && t.visible && t.text == texts[0][3]) language = t;
                if (t != null && t.text == "Applies after restart.") note = t;
            }
            require(language != null && note != null && cross != null, when + ": the Language heading, its note and the crosshair list there");
            var fov:* = content.getChildByName("_srp_fov");
            var left:Array = [c.s_friends, c._setter_invite, c._setter_message, c._set_censure, c._only_friends, c._no_in_batle,
                c._hide_spam, language, menu];
            if (note.visible) left.push(note);
            // Names on map (the last box) with the map's rows, over Static minimap
            var right:Array = [heading, cross, boxes[0], boxes[1], c._setter_static_show_quests, boxes[2], c._setter_static_minimap,
                c._setter_static_minimap_position, fov];
            var bottom:Number = c.bottomContainer.getBounds(content).bottom;
            for each (var column:Array in [left, right])
            {
                for (i = 0; i < column.length; ++i)
                {
                    require(column[i] != null && column[i].visible, when + ": item " + i + " of a column shown");
                    var here:Array = span(column[i]);
                    require(here[1] <= bottom, when + ": " + column[i].name + " above the window's bottom (" + bottom + "): " + here[1]);
                    if (i + 1 < column.length)
                    {
                        var next:Array = span(column[i + 1]);
                        require(here[1] <= next[0] + 1, when + ": " + column[i].name + " (" + here + ") over " + column[i + 1].name + " (" + next + ")");
                    }
                }
            }
            require(!c.s_cross.visible && !c._setter_static_cross.visible && !c.s_quest_propgress.visible && !c._slider_quest_alpha.visible,
                when + ": no \"Crosshair:\" or \"Quest progress:\" heading, no static crosshair checkbox, no transparency slider");
            // alignment
            require(near(language.x, c.s_friends.x) && near(menu.x, c._setter_invite.x + stock.dropdownX),
                when + ": Language under Friends' edges (its list one further)");
            require(near(heading.x, stock.rightHeading) && near(heading.y, c.s_friends.y), when + ": the HUD heading where Crosshair's was");
            for each (var control:* in boxes.concat([c._setter_static_show_quests, c._setter_static_minimap]))
            {
                require(near(control.x, stock.rightControl), when + ": " + control.name + " at the right controls' x");
            }
            require(near(cross.x, stock.rightControl + stock.dropdownX) && near(c._setter_static_minimap_position.x, stock.rightControl + stock.dropdownX),
                when + ": the right column's lists one further");
            // spacing: as the tab's own
            require(near(language.y - c._hide_spam.y, stock.gap) && near(menu.y - language.y, stock.leftFirst),
                when + ": Language the tab's gap under Friends, its list as Friends' first checkbox");
            // the field of view, the right column's last group: the stock slider (its size), its label a heading
            // at the headings' x, the tab's gap under the minimap list (as Language under Friends' last row),
            // in the headings' look: Language's format (font, size, colour, weight) and filters
            require(near(fov.x + fov.textField.x, stock.rightHeading)
                && near(fov.y + fov.textField.y, c._setter_static_minimap_position.y + stock.gap)
                && near(fov.slider.width, new (loader.contentLoaderInfo.applicationDomain.getDefinition("SrvOptionSlider") as Class)().slider.width)
                && fov.scaleX == 1 && fov.scaleY == 1 && near(fov.slider.x, 17),
                when + ": the field of view the stock slider, its heading the tab's gap under the minimap list (" + fov.y + ")");
            var mine:Object = fov.textField.getTextFormat(), theirs:Object = language.getTextFormat();
            require(mine.color == theirs.color && mine.size == theirs.size && mine.font == theirs.font && mine.bold == theirs.bold
                && fov.textField.embedFonts == language.embedFonts && fov.textField.filters.length == language.filters.length,
                when + ": the field of view's heading in Language's look (colour " + Number(mine.color).toString(16) + " / "
                + Number(theirs.color).toString(16) + ", size " + mine.size + " / " + theirs.size + ", font " + mine.font + " / "
                + theirs.font + ", filters " + fov.textField.filters.length + " / " + language.filters.length + ")");
            require(near(cross.y - heading.y, stock.rightFirst), when + ": the crosshair list under the heading as Crosshair's first control was");
            var steps:Array = [cross, boxes[0], boxes[1], c._setter_static_show_quests, boxes[2], c._setter_static_minimap];
            for (i = 1; i < steps.length; ++i)
            {
                require(near(steps[i].y - steps[i - 1].y, stock.step), when + ": the HUD rows a step apart (" + steps[i].name + ")");
            }
            require(near(c._setter_static_minimap_position.y - c._setter_static_minimap.y, stock.dropdown),
                when + ": the minimap list as the timeline has it");
            require(!c.bottomContainer.visible, when + ": no bottom panel across the columns");
            // every dictionary's texts inside their columns
            var leftEnd:Number = stock.rightHeading - 10, rightEnd:Number = stock.rightHeading + c.s_friends.width;
            var labelled:Array = [[c._setter_static_show_quests, 5], [c._setter_static_minimap, 9]];
            for (i = 0; i < boxes.length; ++i)
            {
                labelled.push([boxes[i], LABELS[i]]);
            }
            for each (var row:Array in texts)
            {
                for each (var pair:Array in labelled)
                {
                    var shown:String = pair[0].label;
                    pair[0].label = row[pair[1]];
                    pair[0].validateNow();
                    var end:Number = pair[0].x + pair[0].textField.x + pair[0].textField.textWidth;
                    pair[0].label = shown;
                    pair[0].validateNow();
                    require(end < rightEnd, when + ": " + row[pair[1]] + " ends in the right column (" + rightEnd + "): " + end);
                }
                var was:String = heading.text;
                heading.text = row[2];
                require(heading.x + heading.textWidth < rightEnd, when + ": " + row[2] + " ends in the right column");
                heading.text = was;
                was = language.text;
                language.text = row[3];
                require(language.x + language.textWidth < leftEnd, when + ": " + row[3] + " ends in the left column");
                language.text = was;
                was = fov.textField.text;
                fov.textField.text = row[29] + ":";
                require(fov.textField.textWidth + 4 <= fov.textField.width
                    && fov.x + fov.textFieldValue.x + fov.textFieldValue.width < rightEnd,
                    when + ": " + row[29] + " fits the field of view's heading, its value in the right column");
                fov.textField.text = was;
                if (note.visible)
                {
                    was = note.text;
                    note.text = row[4];
                    require(note.y + note.textHeight + 4 <= bottom, when + ": the note (" + row[4] + ") above the window's bottom: "
                        + (note.y + note.textHeight + 4));
                    require(note.x + note.textWidth < leftEnd, when + ": the note (" + row[4] + ") ends in the left column");
                    note.text = was;
                }
            }
            trace("SETTINGS layout " + when + ": left Friends " + c.s_friends.y + ", Language " + language.y + " (list " + menu.y
                + (note.visible ? ", note " + note.y : "") + ", to " + span(left[left.length - 1])[1]
                + "); right HUD " + heading.y
                + " (crosshair " + cross.y + ", " + boxes.map(function(b:*, ...rest):Number { return b.y; }).join(", ") + ", quests " + c._setter_static_show_quests.y
                + ", minimap " + c._setter_static_minimap.y + ", position " + c._setter_static_minimap_position.y
                + ", field of view " + (fov.y + fov.textField.y) + ", to "
                + span(right[right.length - 1])[1] + "); bottom " + bottom);
        }

        // A player's pick in a list: the list open, the choice, closed (a CLIK dropdown tells its change
        // only while open).
        private static function pick(list:*, index:int):void
        {
            list.open();
            list.selectedIndex = index;
            list.close();
        }

        // The crosshair list's choice and the stock static crosshair checkbox behind it.
        private function crosshair(content:MovieClip):Array
        {
            var cross:* = content.getChildByName("_srp_crosshair");
            return [cross.selectedIndex, Object(content)._setter_static_cross.selected];
        }

        // root.srp_hud_texts as patch 83 answers it (its TEXTS), in the first dictionary.
        private function hudTexts(root:*):void
        {
            root.srp_hud_texts(texts[0][2], texts[0][5], texts[0][6], texts[0][7], texts[0][8], texts[0][9], texts[0][15],
                texts[0][16], texts[0][4], texts[0][19]);
        }

        // The Vulkan options as patch 83 names them, in the panel's order, and their values without a user.cfg
        // line (Prefer the dedicated GPU on).
        private static const VULKAN:Array = ["srp_dxvk_frame_limit", "srp_dxvk_overlay", "srp_dxvk_low_latency",
            "srp_dxvk_tear_free", "srp_dxvk_dedicated_gpu"];
        private static const VULKAN_DEFAULTS:Array = ["0", "0", "0", "0", "1"];

        // A Vulkan option's answer as patch 83 gives it: name, label, saved, running[, the list's choices].
        private function vulkanAnswer(i:int, saved:String, running:String, row:Array = null):void
        {
            var t:Array = row != null ? row : texts[0];
            var root:* = loader.content;
            if (i == 0) root.srp_check_option(VULKAN[0], t[20], saved, running, t[21], "60", "120", "144", "165", "240");
            else if (i == 1) root.srp_check_option(VULKAN[1], t[25], saved, running, t[21], t[26], t[27], t[28]);
            else root.srp_check_option(VULKAN[i], t[20 + i], saved, running);
        }

        // The Graphics tab as the exe makes it: its 19 options (English labels, values as a game's), then
        // the Renderer list's answers (`saved`: "1" Vulkan) and the Vulkan options' (saved and running).
        private function graphicsTab(settings:*, saved:String, running:String = "0", options:Array = null,
            runningOptions:Array = null):void
        {
            var root:* = loader.content;
            var labels:Array = ["Monitor", "Resolution", "Fullscreen", "Vertical sync", "Antialiasing", "Texture filtering",
                "Gamma:", "Field of view", "Overall", "Textures", "Geometry", "Shadows", "Lighting", "Water", "Decorations",
                "Post process", "Ambient occlusion", "Particles", "Motion blur"];
            var list:Array = [], values:Array = [];
            for (var i:int = 0; i < labels.length; ++i)
            {
                var option:Object = {label: labels[i], setter: function(value:*):* { return value; }};
                if (i == 0) option.dataProvider = ["Generic PnP Monitor"];
                else if (i == 1) option.dataProvider = ["1920x1080", "1280x720"];
                else if (i == 4) option.dataProvider = ["Off", "2x", "4x"];
                else if (i == 5) option.dataProvider = ["Bilinear", "Trilinear", "Anisotropic"];
                else if (i != 2 && i != 3 && i != 6 && i != 7) option.dataProvider = ["Low", "Medium", "High"];
                list.push(option);
                values.push({id: i, value: i < 2 || i == 3 ? 0 : i == 2 ? 1 : i == 6 || i == 7 ? 0 : 2});
            }
            try
            {
                settings.selectIndex(2);
            }
            catch (error:Error)
            {
                // the tab's change calls the exe, which a test player lacks
            }
            settings.settings_content.gotoAndStop(3);
            root.set_settings(2, list);
            root.set_values(2, values);
            hudTexts(root);
            root.srp_check_option("srp_dxvk", texts[0][14], saved, running);
            for (var o:int = 0; o < VULKAN.length; ++o)
            {
                vulkanAnswer(o, (options || VULKAN_DEFAULTS)[o], (runningOptions || options || VULKAN_DEFAULTS)[o]);
            }
            // the gamma slider's label, which the game sets through the timeline's alias (its FLA text otherwise)
            settings.settings_content.slider_gamma.textField.text = "Gamma:";
        }

        // The Graphics tab's Renderer list (patch 83's srp_dxvk, docs/notes/dxvk-option.md): shown there
        // only, once the exe answers, built as Monitor's (its label on its left, the same size), one of
        // Monitor's steps under it, the Vulkan options' row a step under that; the rows under them two steps and
        // the note's room down, in order without overlaps, no field of view (the Gameplay tab's), the preset
        // buttons side by side inside their panel, as on main; every dictionary's label and choices
        // inside the list's fields; a pick pending until Apply, Cancel dropping it. Ends back on the
        // gameplay tab.
        // The Renderer note's room (SrpRendererChoice.NOTE_ROOM).
        private static const NOTE_ROOM:Number = 20;

        // The open list of the stage (a dropdown's OptionScrollingList, PopUpManager's), or null.
        private function openList(at:DisplayObjectContainer):DisplayObject
        {
            for (var i:int = 0; i < at.numChildren; ++i)
            {
                var child:DisplayObject = at.getChildAt(i);
                if (child.visible && getQualifiedClassName(child).indexOf("OptionScrollingList") >= 0) return child;
                var inner:DisplayObjectContainer = child as DisplayObjectContainer;
                var found:DisplayObject = inner != null ? openList(inner) : null;
                if (found != null) return found;
            }
            return null;
        }

        // The Renderer's note (the Language choice's "Applies after restart."): in its room under the list,
        // at the list's label, in the column, over nothing, under both lists' open popups; shown and hidden
        // the rows stay where they are; every dictionary's note fits its width.
        private function checkNote(settings:*, content:MovieClip, list:*, monitor:*):void
        {
            var c:* = content;
            var note:TextField = content.getChildByName("_srp_renderer_note") as TextField;
            var rows:Array = [c._fscr_resolution, c._fullscreen, c._vsync, c._antialias, c._filtering, c.slider_gamma];
            var hidden:Array = rows.map(function(r:*, ...rest):Number { return r.y; });
            require(note != null && !note.visible, "note: hidden while the saved choice is the running one");
            graphicsTab(settings, "1", "0");
            require(note.visible && note.text == texts[0][4], "note: shown while Vulkan is saved and DirectX 11 runs");
            var shown:Array = rows.map(function(r:*, ...rest):Number { return r.y; });
            require(hidden.join() == shown.join(), "note: the rows stay where they are (" + hidden + " / " + shown + ")");
            // the Vulkan options' row's bottom: a step under the Renderer list, a list's height
            var listBottom:Number = list.y + (list.y - monitor.y) + list.height;
            var resolutionTop:Number = span(c._fscr_resolution)[0];
            var columnEnd:Number = monitor.x + monitor.width;
            var widest:Number = 0;
            for each (var row:Array in texts)
            {
                note.text = row[4];
                require(note.textWidth + 4 <= note.width && note.x + note.width <= columnEnd + 0.1, "note: " + row[4]
                    + " fits the column (" + note.textWidth + " of " + note.width + ", to " + (note.x + note.width) + ")");
                var top:Number = note.y, bottom:Number = note.y + note.textHeight + 4;
                require(top >= listBottom - 0.1 && bottom <= resolutionTop + 0.1, "note: " + row[4] + " between the Vulkan options ("
                    + listBottom + ") and Resolution (" + resolutionTop + "): " + top + "-" + bottom);
                widest = Math.max(widest, note.textWidth);
            }
            require(near(note.x, list.x + list.textFieldLabel.x), "note: at the list's label");
            // the open lists against the widest dictionary's note
            var longest:String = texts[0][4];
            for each (row in texts)
            {
                note.text = row[4];
                if (note.textWidth > widest - 0.01) longest = row[4];
            }
            note.text = longest;
            // the open lists: the Renderer's under its box (where Monitor's opens), both over the note
            var Point:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("flash.geom.Point") as Class;
            var lefts:Array = [];
            for each (var dropdown:* in [monitor, list])
            {
                dropdown.open();
                var open:DisplayObject = openList(content.stage);
                require(open != null, "note: " + dropdown.name + "'s list opens");
                var bounds:Rectangle = open.getBounds(content.stage);
                lefts.push(bounds.left);
                // the note's text where it meets the open list: the list on top there, or apart
                var drawn:Rectangle = new Rectangle(note.x, note.y, note.textWidth + 4, note.textHeight + 4);
                var topLeft:* = content.localToGlobal(new Point(drawn.x, drawn.y));
                var meet:Rectangle = new Rectangle(topLeft.x, topLeft.y, drawn.width, drawn.height).intersection(bounds);
                if (!meet.isEmpty())
                {
                    var under:Array = content.stage.getObjectsUnderPoint(new Point(meet.x + meet.width / 2, meet.y + meet.height / 2));
                    var topmost:DisplayObject = under.length > 0 ? under[under.length - 1] : null;
                    var inList:Boolean = false;
                    for (var o:DisplayObject = topmost; o != null; o = o.parent) if (o == open) inList = true;
                    require(inList, "note: " + dropdown.name + "'s open list (" + bounds + ") is over the note where they meet");
                }
                trace("SETTINGS Graphics: " + dropdown.name + "'s open list " + bounds + (meet.isEmpty() ? ", apart from the note" : ", over the note") + " (" + note.text + ")");
                dropdown.close();
                require(openList(content.stage) == null, "note: " + dropdown.name + "'s list closes");
            }
            require(Math.abs(lefts[0] - lefts[1]) < 1, "note: the Renderer's list opens at Monitor's box (" + lefts + ")");
            note.text = texts[0][4];
            graphicsTab(settings, "0", "0");
            require(!note.visible && rows.map(function(r:*, ...rest):Number { return r.y; }).join() == hidden.join(),
                "note: hidden again, the rows where they were");
            trace("SETTINGS Graphics: the note between " + listBottom + " and " + resolutionTop + ", the widest " + widest
                + " of " + note.width + "; both lists open at x " + lefts);
        }

        private function checkGraphics(settings:*, content:MovieClip, gameplay:Array, gameplayValues:Array):void
        {
            var root:* = loader.content;
            var list:* = content.getChildByName("_srp_renderer");
            require(list != null && !list.visible, "Renderer: not on the gameplay tab");
            var fresh:* = new (Object(content).constructor as Class)();
            fresh.gotoAndStop(3);
            graphicsTab(settings, "0");
            var c:* = content;
            require(c.currentFrame == 3 && list.visible && list.alpha == 1 && list.textFieldLabel.text == texts[0][14]
                && list.selectedIndex == 0 && list.dataProvider.length == 2 && list.dataProvider[0] == texts[0][15]
                && list.dataProvider[1] == texts[0][16] && !settings.btn_accept.enabled,
                "Renderer: on the Graphics tab, the exe's label and choices, DirectX 11, nothing pending");
            var step:Number = fresh._fscr_resolution.y - fresh._select_monitor.y;
            var monitor:* = c._select_monitor;
            require(near(list.x, monitor.x) && near(list.y, monitor.y + step) && Math.abs(list.width - monitor.width) < 1
                && Math.abs(list.height - monitor.height) < 1, "Renderer: Monitor's size, one of its steps under it ("
                + list.width + "x" + list.height + " at " + list.y + ")");
            // the rows under it: two steps (the Vulkan options' row) and the note's room down, kept whether
            // the note shows or not
            for each (var moved:String in ["_fscr_resolution", "_fullscreen", "_vsync", "_antialias", "_filtering", "slider_gamma"])
            {
                require(near(c[moved].y, fresh[moved].y + 2 * step + NOTE_ROOM), "Renderer: " + moved + " two steps and the note's room down");
            }
            checkNote(settings, content, list, monitor);
            var column:Array = [c._select_monitor, list, c.getChildByName("_srp_vulkan"), c._fscr_resolution, c._fullscreen, c._vsync, c._antialias, c._filtering,
                c.slider_gamma];
            // the field of view is the Gameplay tab's: none here, and no control of the tab named for the video
            // options' dead option 7
            var fovHere:* = c.getChildByName("_srp_fov");
            require(c.getChildByName("_slider_fov") == null && (fovHere == null || !fovHere.visible),
                "Graphics: no field of view slider (the Gameplay tab's)");
            var panel:Rectangle = c.bottomContainer.getBounds(content);
            for (var i:int = 0; i < column.length; ++i)
            {
                var here:Array = span(column[i]);
                require(column[i].visible, "Graphics: " + column[i].name + " shown");
                var next:Number = i + 1 < column.length ? span(column[i + 1])[0] : panel.top;
                require(here[1] <= next + 1, "Graphics: " + column[i].name + " (" + here + ") over the next (" + next + ")");
            }
            // the buttons draw at their new width in the next frame: now, for the measure
            c.button_optimal_video.validateNow();
            c.button_default_video.validateNow();
            var optimal:Rectangle = c.button_optimal_video.getBounds(content);
            var standard:Rectangle = c.button_default_video.getBounds(content);
            // the preset panel where it was before the Vulkan options (its top, size and buttons main's)
            require(c.bottomContainer.visible && panel.containsRect(optimal) && panel.containsRect(standard)
                && optimal.right < standard.left && near(optimal.y, standard.y), "Graphics: the preset buttons side by "
                + "side inside their panel (" + optimal + ", " + standard + " in " + panel + ")");
            require(near(panel.bottom, fresh.bottomContainer.getBounds(fresh).bottom)
                && near(panel.top, fresh.bottomContainer.getBounds(fresh).top + 50 + step + NOTE_ROOM),
                "Graphics: the panel's top and bottom main's (" + panel + ")");
            var labelEnd:Number = list.x + list.textFieldLabel.x + list.textFieldLabel.width;
            var widestLabel:Number = 0, widestChoice:Number = 0;
            var choices:Object = list.dataProvider;
            for each (var row:Array in texts)
            {
                list.textFieldLabel.text = row[14];
                require(list.textFieldLabel.textWidth + 4 < list.textFieldLabel.width, "Renderer: " + row[14]
                    + " fits its label (" + list.textFieldLabel.textWidth + " of " + list.textFieldLabel.width + ")");
                widestLabel = Math.max(widestLabel, list.textFieldLabel.textWidth);
                list.dataProvider = new (loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.data.DataProvider")
                    as Class)([row[15], row[16]]);
                for (var choice:int = 0; choice < 2; ++choice)
                {
                    list.selectedIndex = choice;
                    list.validateNow();
                    // Monitor's list leaves its field a little room: within half its 2-pixel gutters the
                    // text is drawn whole (the screenshot's "Vulkan/DXVK")
                    require(list.textField.textWidth + 2 < list.textField.width * list.scaleX, "Renderer: " + row[15 + choice]
                        + " fits the list (" + list.textField.textWidth + " of " + list.textField.width * list.scaleX + ")");
                    widestChoice = Math.max(widestChoice, list.textField.textWidth);
                }
            }
            list.textFieldLabel.text = texts[0][14];
            list.dataProvider = choices;
            list.selectedIndex = 0;
            list.validateNow();
            require(!settings.btn_accept.enabled, "Renderer: nothing pending after the texts' check");
            trace("SETTINGS Graphics: Renderer at " + list.y + " under Monitor " + monitor.y + ", gamma to " + span(c.slider_gamma)[1]
                + ", panel " + panel.top + "-" + panel.bottom + "; widest label " + widestLabel + ", widest choice " + widestChoice);
            pick(list, 1);
            require(settings.btn_accept.enabled, "Renderer: picking Vulkan enables Apply");
            click(settings.btn_cancel);
            require(list.selectedIndex == 0 && !settings.btn_accept.enabled, "Renderer: Cancel puts DirectX 11 back");
            pick(list, 1);
            click(settings.btn_accept);
            require(list.selectedIndex == 1 && !settings.btn_accept.enabled, "Renderer: Apply saves Vulkan");
            var note:* = content.getChildByName("_srp_renderer_note");
            require(note.visible, "Renderer: the note after Apply of Vulkan (DirectX 11 running)");
            root.srp_check_option("srp_dxvk", texts[0][14], "1", "0");
            require(list.selectedIndex == 1 && !settings.btn_accept.enabled && note.visible,
                "Renderer: the exe's answer (on, DirectX running) keeps it, the note shown");
            pick(list, 0);
            require(settings.btn_accept.enabled, "Renderer: DirectX 11 over a saved Vulkan is a change");
            click(settings.btn_cancel);
            require(list.selectedIndex == 1, "Renderer: Cancel: Vulkan again");
            root.srp_check_option("srp_dxvk", texts[0][14], "0", "0");
            require(list.selectedIndex == 0 && !settings.btn_accept.enabled && !note.visible,
                "Renderer: the exe's answer sets the saved choice; saved as running, no note");
            root.srp_check_option("srp_dxvk", texts[0][14], "1", "1");
            require(!note.visible, "Renderer: Vulkan saved and running: no note");
            pick(list, 0);
            click(settings.btn_accept);
            require(note.visible, "Renderer: Apply of DirectX 11 while DXVK runs: the note");
            root.srp_check_option("srp_dxvk", texts[0][14], "0", "0");
            graphicsTab(settings, "0");
            require(list.visible && near(list.y, monitor.y + step) && near(c._fscr_resolution.y, fresh._fscr_resolution.y + 2 * step + NOTE_ROOM),
                "Renderer: the tab bound again, every row moved once");
            require(!settings.btn_accept.enabled, "Renderer: nothing pending after the checks");
            trace("SETTINGS Graphics: Renderer pass");
            checkVulkan(settings, content, step);
            trace("SETTINGS Graphics: Vulkan options pass");
            try
            {
                settings.selectIndex(0);
            }
            catch (error:Error)
            {
            }
            content.gotoAndStop(1);
            root.set_settings(0, gameplay);
            root.set_values(0, gameplayValues);
            require(!list.visible && !settings.btn_accept.enabled, "Renderer: not on the gameplay tab again, nothing pending");
        }

        // The stage's display object of that name (a popup's: PopUpManager's canvas), or null.
        private function named(at:DisplayObjectContainer, name:String):DisplayObject
        {
            for (var i:int = 0; i < at.numChildren; ++i)
            {
                var child:DisplayObject = at.getChildAt(i);
                if (child.name == name) return child;
                var inner:DisplayObjectContainer = child as DisplayObjectContainer;
                var found:DisplayObject = inner != null ? named(inner, name) : null;
                if (found != null) return found;
            }
            return null;
        }

        // The Vulkan options (docs/notes/dxvk-option.md): the button a step under the Renderer list, across
        // its row (the preset buttons' SetingsButton), disabled while the list shows DirectX 11 and enabled by
        // a Vulkan pick before Apply; a press opens the panel right under it, its edges the button's, over the
        // column: the two lists (the exe's choices) and the three checkboxes on one step, in order, each
        // showing its saved value, every dictionary's texts inside; a press in it or on its open list keeps
        // it, one outside, the button again or Esc closes it (the window stays),
        // DirectX 11 picked closes it; a change pending until Apply, Cancel dropping it; the note while a
        // saved value is not the running one.
        private function checkVulkan(settings:*, content:MovieClip, step:Number):void
        {
            var root:* = loader.content;
            var c:* = content;
            var list:* = c.getChildByName("_srp_renderer");
            var row:* = c.getChildByName("_srp_vulkan");
            var note:* = c.getChildByName("_srp_renderer_note");
            graphicsTab(settings, "0");
            row.validateNow();
            var optimal:* = c.button_optimal_video;
            require(row.visible && row.label == texts[0][19] && getQualifiedClassName(row) == getQualifiedClassName(optimal)
                && !row.enabled && !settings.btn_accept.enabled,
                "Vulkan options: the button (" + getQualifiedClassName(row) + ", the preset buttons' " + getQualifiedClassName(optimal)
                + ") with the exe's text, disabled while the Renderer shows DirectX 11, nothing pending");
            require(near(row.x, list.x) && Math.abs(row.width - list.width) < 1
                && near(row.y + row.height / 2, list.y + step + list.height / 2),
                "Vulkan options: the button across the Renderer's row, one step under it (" + row.x + "," + row.y + " "
                + row.width + "x" + row.height + ")");
            click(row);
            require(named(content.stage, "_srp_vulkan_panel") == null, "Vulkan options: a press of the disabled button opens nothing");
            pick(list, 1);
            row.validateNow();
            require(row.enabled && settings.btn_accept.enabled, "Vulkan options: enabled by a Vulkan pick before Apply");
            click(row);
            var panel:* = named(content.stage, "_srp_vulkan_panel");
            require(panel != null && panel.stage != null, "Vulkan options: the button opens the panel");
            var body:* = panel.getChildAt(1);
            var controls:Array = VULKAN.map(function(name:String, ...rest):* { return body.getChildByName("_" + name); });
            for each (var control:* in controls) control.validateNow();
            var Point:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("flash.geom.Point") as Class;
            // the panel as drawn: its background, less the half pixel its border adds on each side (a checkbox's
            // empty room may reach further, unseen)
            var bounds:Rectangle = panel.getChildAt(0).getBounds(content.stage);
            bounds.inflate(-0.5, -0.5);
            var rowBounds:Rectangle = row.getBounds(content.stage);
            var divider:Number = content.localToGlobal(new Point(c._overall_graphics.x - 20, 0)).x;
            require(Math.abs(bounds.top - rowBounds.bottom) < 1 && Math.abs(bounds.left - rowBounds.left) < 1
                && Math.abs(bounds.right - rowBounds.right) < 1 && bounds.right < divider,
                "Vulkan options: the panel right under the button, its edges the button's, over the column ("
                + bounds + " under " + rowBounds + ", the right column at " + divider + ")");
            for each (var underRow:* in [c._fscr_resolution, c._fullscreen, c._vsync, c._antialias, c._filtering,
                c.slider_gamma])
            {
                var rowSpan:Rectangle = underRow.getBounds(content.stage);
                require(!(rowSpan.top < bounds.bottom && rowSpan.bottom > bounds.bottom), "Vulkan options: the panel ("
                    + bounds + ") ends on a row's boundary, " + underRow.name + " (" + rowSpan + ") not half under it");
            }
            var labels:Array = [texts[0][20], texts[0][25], texts[0][22], texts[0][23], texts[0][24]];
            var at:Number = -1;
            for (var i:int = 0; i < controls.length; ++i)
            {
                var one:* = controls[i];
                var label:String = i < 2 ? one.textFieldLabel.text : one.label;
                require(one != null && one.visible && one.alpha == 1 && label == labels[i], "Vulkan options: " + VULKAN[i]
                    + " in the panel with the exe's label (" + label + ")");
                var top:Number = span(one)[0];
                require(top > at && near(one.x, controls[0].x), "Vulkan options: " + VULKAN[i] + " in order, at the panel's left");
                at = span(one)[1];
                require(at <= panel.height, "Vulkan options: " + VULKAN[i] + " inside the panel (" + at + " of " + panel.height + ")");
            }
            require(controls[0].dataProvider.length == 6 && controls[0].dataProvider[0] == texts[0][21]
                && controls[0].dataProvider[3] == "144" && controls[1].dataProvider.length == 4
                && controls[1].dataProvider[2] == texts[0][27] && controls[0].selectedIndex == 0 && controls[1].selectedIndex == 0
                && !controls[2].selected && !controls[3].selected && controls[4].selected,
                "Vulkan options: the lists' choices; each control its saved value (Prefer the dedicated GPU on)");
            var rhythm:Number = controls[1].y - controls[0].y;
            for (i = 1; i < controls.length; ++i)
            {
                require(near(controls[i].y - controls[i - 1].y, rhythm), "Vulkan options: the five rows on one step ("
                    + VULKAN[i] + ": " + (controls[i].y - controls[i - 1].y) + ", " + rhythm + ")");
            }
            // the background as drawn (its 1-pixel border adds half a pixel on each side)
            var under:Number = panel.getChildAt(0).height - 1 - (controls[4].y + 26);
            require(rhythm >= step - 0.1 && rhythm <= step + 3 && Math.abs(controls[0].width - (list.width - controls[0].x)) < 1
                && Math.abs(under - controls[0].y) < 1.5,
                "Vulkan options: the rows a list's step apart (or a little more, to end on a row's edge), the lists from their inset to the "
                + "panel's right edge, the padding over the first row the padding under the last (" + controls[0].y + ", "
                + under + "; step " + rhythm + ")");
            // every dictionary's texts inside the panel
            var widest:Number = 0;
            var DataProvider:Class = loader.contentLoaderInfo.applicationDomain.getDefinition("scaleform.clik.data.DataProvider") as Class;
            for each (var t:Array in texts)
            {
                vulkanAnswer(0, "0", "0", t);
                vulkanAnswer(1, "0", "0", t);
                for (i = 2; i < controls.length; ++i) vulkanAnswer(i, VULKAN_DEFAULTS[i], VULKAN_DEFAULTS[i], t);
                for (i = 0; i < 2; ++i)
                {
                    var menu:* = controls[i];
                    require(menu.textFieldLabel.textWidth + 4 < menu.textFieldLabel.width, "Vulkan options: " + menu.textFieldLabel.text
                        + " fits its label (" + menu.textFieldLabel.textWidth + " of " + menu.textFieldLabel.width + ")");
                    for (var choice:int = 0; choice < menu.dataProvider.length; ++choice)
                    {
                        menu.selectedIndex = choice;
                        menu.validateNow();
                        require(menu.textField.textWidth + 2 < menu.textField.width * menu.scaleX, "Vulkan options: "
                            + menu.dataProvider[choice] + " fits its list (" + menu.textField.textWidth + ")");
                    }
                    menu.selectedIndex = 0;
                }
                for (i = 2; i < controls.length; ++i)
                {
                    var box:* = controls[i];
                    box.validateNow();
                    var end:Number = box.x + box.textField.x + box.textField.textWidth + 4;
                    require(box.textField.numLines == 1 && end <= panel.width, "Vulkan options: " + box.label
                        + " on one line inside the panel (to " + end + " of " + panel.width + ")");
                    widest = Math.max(widest, end);
                }
                row.label = t[19];
                row.validateNow();
                require(row.textField.textWidth + 4 <= row.textField.width * row.scaleX, "Vulkan options: " + t[19]
                    + " fits its button (" + row.textField.textWidth + " of " + row.textField.width * row.scaleX + ")");
            }
            // Vulkan saved and running (DXVK at the start), so only an option makes the note
            graphicsTab(settings, "1", "1");
            hudTexts(root);
            require(!settings.btn_accept.enabled && list.selectedIndex == 1 && row.enabled, "Vulkan options: the texts' check left nothing pending");
            // presses: in the panel and on its open list keep it, outside closes it
            controls[2].dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN, true));
            require(panel.stage != null, "Vulkan options: a press in the panel keeps it");
            controls[0].open();
            var open:DisplayObject = openList(content.stage);
            require(open != null, "Vulkan options: the frame rate list opens");
            open.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN, true));
            require(panel.stage != null, "Vulkan options: a press on the panel's open list keeps the panel");
            controls[0].close();
            c.slider_gamma.dispatchEvent(new MouseEvent(MouseEvent.MOUSE_DOWN, true));
            require(panel.stage == null, "Vulkan options: a press outside closes the panel");
            click(row);
            click(row);
            require(panel.stage == null, "Vulkan options: the button again closes it");
            // Esc (MainMenu's key handler asks Settings.closePanel first; a test player's stage takes no
            // key event dispatched to it): the panel closes, the window stays; with no panel open nothing
            click(row);
            require(settings.closePanel() && panel.stage == null && settings.visible,
                "Vulkan options: Esc closes the panel, the window stays");
            require(!settings.closePanel(), "Vulkan options: Esc with no panel open is the window's");
            // DirectX 11 picked while it is open: closed, greyed
            click(row);
            pick(list, 0);
            row.validateNow();
            require(panel.stage == null && !row.enabled, "Vulkan options: DirectX 11 picked closes the panel and greys the row");
            pick(list, 1);
            // a change pending until Apply, Cancel dropping it
            require(!settings.btn_accept.enabled, "Vulkan options: nothing pending before a change");
            controls[2].selected = true;
            require(settings.btn_accept.enabled, "Vulkan options: Reduce input lag checked is a change");
            click(settings.btn_cancel);
            require(!controls[2].selected && !settings.btn_accept.enabled, "Vulkan options: Cancel unchecks it");
            // in the open panel (a list opens on the stage only)
            click(row);
            pick(controls[0], 3);
            controls[4].selected = false;
            settings.closePanel();
            require(settings.btn_accept.enabled && !note.visible, "Vulkan options: 144 and the dedicated GPU off are a change, no note yet");
            click(settings.btn_accept);
            require(!settings.btn_accept.enabled && controls[0].selectedIndex == 3 && !controls[4].selected && note.visible,
                "Vulkan options: Apply saves them; the note (saved is not what runs)");
            // the exe's answers: what it saved, and running DXVK with what the start had
            graphicsTab(settings, "1", "1", ["3", "0", "0", "0", "0"], ["0", "0", "0", "0", "1"]);
            require(note.visible && controls[0].selectedIndex == 3 && !controls[4].selected,
                "Vulkan options: DXVK running with other options: the note");
            graphicsTab(settings, "1", "1", ["3", "0", "0", "0", "0"]);
            require(!note.visible, "Vulkan options: saved as running: no note");
            graphicsTab(settings, "0");
            require(!row.enabled && !note.visible && !settings.btn_accept.enabled && panel.stage == null,
                "Vulkan options: back to DirectX 11, nothing pending");
            trace("SETTINGS Graphics: Vulkan options' panel " + bounds + " under the row " + rowBounds + "; widest checkbox to "
                + widest + " of " + panel.width);
        }

        private function check():void
        {
            // [[hit markers' label, scores' label, the group's heading, "Language:", its restart note, "Hide quests",
            //   the crosshair list's three choices, "Static minimap", the minimap position list's three
            //   (st_minimap_off, _bottom, _top)], ...] for every dictionary
            texts = JSON.parse(loaderInfo.parameters.texts) as Array;
            var root:* = loader.content;
            var settings:* = root.settings_window;
            var content:MovieClip = settings.settings_content;
            var c:* = content;
            var domain:ApplicationDomain = loader.contentLoaderInfo.applicationDomain;
            // the tab's own headings as the exe names them (st_options_friends, ...)
            try
            {
                root.set_labels([{name: "gameplay_options_type", label: "Gameplay"}, {name: "controllers_options_type", label: "Controls"},
                    {name: "video_options_type", label: "Graphics"}, {name: "sound_options_type", label: "Sound"},
                    {name: "button_optimal_video", label: "Optimal"}, {name: "button_default_video", label: "Default"},
                    {name: "settings_label", label: "Settings"}, {name: "button_apply", label: "Apply"},
                    {name: "button_exit", label: "Exit"}, {name: "button_cancel", label: "Cancel"},
                    {name: "s_friends", label: "Friends:"}, {name: "s_cross", label: "Crosshair:"},
                    {name: "s_quest_propgress", label: "Quest progress:"}]);
            }
            catch (error:Error)
            {
                // the tab bar's change calls the exe, which a test player lacks
            }
            // the exe's settings: the stock English labels, each with a setter that records what the
            // menu gives the exe (and gives it back, as the game's do)
            var given:Array = [];
            var settingsList:Array = [];
            var values:Array = [];
            for (var i:int = 0; i < 16; ++i)
            {
                settingsList.push({label: STOCK_LABELS[i] || "option " + i, setter: setterOf(i, given)});
                values.push({id: i, value: 0});
            }
            // the stock defaults: quest progress shown (10), the transparency at 1 (11); the crosshair static (7)
            values[10].value = 1;
            values[11].value = 1;
            values[7].value = 1;
            // the minimap position list's choices (st_minimap_off, _bottom, _top), the lower left corner
            settingsList[9].dataProvider = [texts[0][10], texts[0][11], texts[0][12]];
            values[9].value = 1;
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            root.srp_language(texts[0][3], "Applies after restart.", "english", "english", "english", "English",
                "ukrainian", "Українська");
            root.show_settings(true);

            var boxes:Array = [];
            for each (var name:String in NAMES)
            {
                var box:* = content.getChildByName("_" + name);
                require(box != null && !box.visible, name + ": the checkbox waits for the exe's answer");
                boxes.push(box);
            }
            var cross:* = content.getChildByName("_srp_crosshair");
            require(cross != null && !cross.visible, "the crosshair list waits for the exe's answers");
            for (i = 0; i < NAMES.length; ++i)
            {
                root.srp_check_option(NAMES[i], texts[0][LABELS[i]], "0");
                require(boxes[i].visible && boxes[i].alpha == 1, NAMES[i] + ": on the gameplay tab");
                require(boxes[i].label == texts[0][LABELS[i]], NAMES[i] + ": its label is the exe's: " + boxes[i].label);
                require(!boxes[i].selected, NAMES[i] + ": the saved state (off) is shown");
            }
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(!cross.visible, "the crosshair list waits for its texts");
            hudTexts(root);
            require(!settings.btn_accept.enabled, "nothing to apply after the answers");
            var heading:TextField = content.getChildByName("_srp_check_group") as TextField;
            require(heading != null && heading.visible && heading.text == texts[0][2], "the group's heading is the exe's: " + (heading ? heading.text : null));
            require(cross.visible && cross.dataProvider.length == 3 && cross.dataProvider[0] == texts[0][6]
                && cross.dataProvider[1] == texts[0][7] && cross.dataProvider[2] == texts[0][8], "the crosshair list: the three choices");
            require(c._setter_static_show_quests.label == texts[0][5] && c._setter_static_minimap.label == texts[0][9],
                "the stock quest progress and minimap checkboxes labelled Hide quests and Static minimap");
            root.srp_check_option("srp_no_such", "x", "1");
            require(!boxes.some(function(b:*, ...rest):Boolean { return b.selected; }) && !settings.btn_accept.enabled,
                "another name changes nothing");

            // the field of view (patch 83's uro_fov): shown once the exe answers, the stock slider in its range
            var fovSlider:* = content.getChildByName("_srp_fov");
            require(fovSlider != null && !fovSlider.visible, "the field of view waits for the exe's answer");
            root.srp_slider_option("uro_fov", texts[0][29], "65", "50", "100");
            require(fovSlider.visible && fovSlider.alpha == 1 && fovSlider.textField.text == texts[0][29] + ":"
                && fovSlider.slider.minimum == 50 && fovSlider.slider.maximum == 100 && fovSlider.selectedIndex == 65
                && !settings.btn_accept.enabled, "the field of view: the exe's label (a colon), range 50 to 100 and value 65, "
                + "nothing pending (" + fovSlider.textField.text + ", " + fovSlider.selectedIndex + ")");
            checkLayout(content, boxes, texts, stockOf(content), "at rest");
            fovSlider.slider.value = 90;
            require(fovSlider.selectedIndex == 90 && settings.btn_accept.enabled, "the field of view moved to 90: a change");
            click(settings.btn_cancel);
            require(fovSlider.selectedIndex == 65 && !settings.btn_accept.enabled, "the field of view: Cancel puts 65 back");
            fovSlider.slider.value = 90;
            click(settings.btn_accept);
            require(fovSlider.selectedIndex == 90 && !settings.btn_accept.enabled, "the field of view: Apply saves 90");
            root.srp_slider_option("uro_fov", texts[0][29], "90", "50", "100");
            require(fovSlider.selectedIndex == 90 && !settings.btn_accept.enabled, "the field of view: the exe's answer keeps 90");
            root.srp_slider_option("uro_fov", texts[0][29], "65", "50", "100");
            root.srp_slider_option("uro_fov", texts[0][29], "65", "100", "50");
            require(fovSlider.selectedIndex == 65 && !settings.btn_accept.enabled, "the field of view: an answer with no range is "
                + "ignored");
            trace("SETTINGS field of view: Gameplay, the stock slider at " + fovSlider.x + "," + fovSlider.y + ", Apply and Cancel");
            // the restart note shown (a saved language not the running one)
            root.srp_language(texts[0][3], "Applies after restart.", "ukrainian", "english", "english", "English",
                "ukrainian", "Українська");
            checkLayout(content, boxes, texts, stockOf(content), "the restart note shown");
            root.srp_language(texts[0][3], "Applies after restart.", "english", "english", "english", "English",
                "ukrainian", "Українська");
            checkLayout(content, boxes, texts, stockOf(content), "the note hidden again");
            var invite:DisplayObject = content["_setter_invite"];
            for each (box in boxes)
            {
                require(Math.abs(box.width - invite.width) < 1 && Math.abs(box.height - invite.height) < 1,
                    box.name + ": the tab's checkbox at its size: " + box.width + "x" + box.height + " (" + invite.width + "x" + invite.height + ")");
            }
            // every dictionary's three choices inside the list
            var choices:Object = cross.dataProvider;
            for each (var row:Array in texts)
            {
                cross.dataProvider = new (domain.getDefinition("scaleform.clik.data.DataProvider") as Class)([row[6], row[7], row[8]]);
                for (i = 0; i < 3; ++i)
                {
                    cross.selectedIndex = i;
                    cross.validateNow();
                    // the field is counter-scaled: its width is in the list's units, its text in the screen's
                    require(cross.textField.textWidth + 4 < cross.textField.width * cross.scaleX, row[6 + i] + " fits the crosshair list ("
                        + cross.textField.textWidth + " of " + cross.textField.width * cross.scaleX + ")");
                }
            }
            // every dictionary's minimap position choices inside their list
            var position:* = c._setter_static_minimap_position;
            var positions:Object = position.dataProvider;
            var shownPosition:int = position.selectedIndex;
            var widest:String = "", widestWidth:Number = 0;
            for each (row in texts)
            {
                position.dataProvider = new (domain.getDefinition("scaleform.clik.data.DataProvider") as Class)([row[10], row[11], row[12]]);
                for (i = 0; i < 3; ++i)
                {
                    position.selectedIndex = i;
                    position.validateNow();
                    require(position.textField.textWidth + 4 < position.textField.width * position.scaleX, row[10 + i]
                        + " fits the minimap position list (" + position.textField.textWidth + " of " + position.textField.width * position.scaleX + ")");
                    if (position.textField.textWidth > widestWidth)
                    {
                        widest = row[10 + i];
                        widestWidth = position.textField.textWidth;
                    }
                }
            }
            trace("SETTINGS minimap position list: every dictionary's choices fit, the widest " + widest + " (" + widestWidth + " of "
                + position.textField.width * position.scaleX + ")");
            position.dataProvider = positions;
            position.selectedIndex = shownPosition;
            position.validateNow();
            hudTexts(root);
            root.set_values(0, values);
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(!settings.btn_accept.enabled, "nothing pending after the texts' check");

            // Hide quests: the stock quest progress inverted
            var quests:* = c._setter_static_show_quests;
            require(!quests.selected, "the stock default (quest progress shown) is Hide quests unchecked");
            values[10].value = 0;
            root.set_values(0, values);
            require(quests.selected && !settings.btn_accept.enabled, "quest progress off (0) is Hide quests checked");
            values[10].value = 1;
            root.set_values(0, values);
            require(!quests.selected, "and on (1) unchecked again");
            given.length = 0;
            quests.selected = true;
            require(given.length == 1 && given[0][0] == 10 && given[0][1] == false && quests.selected && settings.btn_accept.enabled,
                "checking Hide quests gives the exe quest progress off (" + given + "), Apply enabled");
            quests.selected = false;
            require(given[given.length - 1][1] == true && !settings.btn_accept.enabled, "unchecking it gives it on again, nothing pending");
            quests.selected = true;
            click(settings.btn_cancel);
            root.set_values(0, values);
            require(!quests.selected && !settings.btn_accept.enabled, "Cancel: the stock value (shown) back, Hide quests unchecked");
            quests.selected = true;
            click(settings.btn_accept);
            require(quests.selected && !settings.btn_accept.enabled, "Apply keeps Hide quests checked");
            values[10].value = 0;
            root.set_values(0, values);
            require(quests.selected && !settings.btn_accept.enabled, "the exe's value after Apply (off) is Hide quests checked");
            trace("SETTINGS Hide quests: the stock quest progress inverted through the setter, Apply and Cancel");

            // the crosshair list: Dynamic and Static are the stock checkbox, Hidden is patch 83's
            var stockBox:* = c._setter_static_cross;
            require(crosshair(content).join() == "1,true", "the stock static crosshair is Static: " + crosshair(content));
            values[7].value = 0;
            root.set_values(0, values);
            require(crosshair(content).join() == "0,false" && !settings.btn_accept.enabled, "the stock dynamic crosshair is Dynamic");
            values[7].value = 1;
            root.set_values(0, values);
            given.length = 0;
            pick(cross, 0);
            require(crosshair(content).join() == "0,false" && given.length == 1 && given[0].join() == "7,false" && settings.btn_accept.enabled,
                "picking Dynamic unchecks the stock checkbox, given to the exe (" + given + "), Apply enabled");
            click(settings.btn_cancel);
            root.set_values(0, values);
            require(crosshair(content).join() == "1,true" && !settings.btn_accept.enabled, "Cancel: Static back");
            given.length = 0;
            pick(cross, 2);
            require(crosshair(content).join() == "2,true" && given.length == 0 && settings.btn_accept.enabled,
                "picking Hidden leaves the stock checkbox (Static), Apply enabled");
            click(settings.btn_accept);
            require(crosshair(content).join() == "2,true" && !settings.btn_accept.enabled, "Apply saves Hidden");
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "1");
            root.set_values(0, values);
            require(crosshair(content).join() == "2,true" && !settings.btn_accept.enabled, "the exe's answers keep Hidden");
            pick(cross, 0);
            require(crosshair(content).join() == "0,false" && settings.btn_accept.enabled, "from Hidden, Dynamic: the stock checkbox unchecked, pending");
            click(settings.btn_cancel);
            root.set_values(0, values);
            require(crosshair(content).join() == "2,true" && !settings.btn_accept.enabled, "Cancel: Hidden back, the stock value Static kept");
            pick(cross, 1);
            require(crosshair(content).join() == "1,true" && settings.btn_accept.enabled, "from Hidden, Static: pending (Hidden dropped)");
            click(settings.btn_accept);
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(crosshair(content).join() == "1,true" && !settings.btn_accept.enabled, "Apply: Static, Hidden off");
            values[7].value = 0;
            root.set_values(0, values);
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "1");
            require(crosshair(content).join() == "2,false", "saved Hidden over a dynamic stock crosshair: Hidden");
            root.srp_check_option("srp_hide_crosshair", texts[0][8], "0");
            require(crosshair(content).join() == "0,false" && !settings.btn_accept.enabled, "Hidden off again: back to Dynamic, the stock value it kept");
            values[7].value = 1;
            root.set_values(0, values);
            // a transient: it opens on top and a press outside closes it
            cross.open();
            require(cross.selected && content.stage.getObjectsUnderPoint(new (domain.getDefinition("flash.geom.Point") as Class)(cross.localToGlobal(new (domain.getDefinition("flash.geom.Point") as Class)(5, cross.height + 15)).x, cross.localToGlobal(new (domain.getDefinition("flash.geom.Point") as Class)(5, cross.height + 15)).y)).length > 0, "the crosshair list opens (its list under it)");
            stage.dispatchEvent(new (domain.getDefinition("flash.events.MouseEvent") as Class)("mouseDown", true, false, 5, 5));
            if (cross.selected)
            {
                content.stage.dispatchEvent(new (domain.getDefinition("flash.events.MouseEvent") as Class)("mouseDown", true, false, 5, 5));
            }
            require(!cross.selected, "a press outside closes the crosshair list");
            trace("SETTINGS crosshair list: Dynamic/Static the stock checkbox, Hidden patch 83's, Apply and Cancel, closes outside");

            for (i = 0; i < boxes.length; ++i)
            {
                box = boxes[i];
                var others:Array = boxes.filter(function(b:*, ...rest):Boolean { return b != box; });
                box.selected = true;
                require(settings.btn_accept.enabled && settings.btn_cancel.label == "Cancel", NAMES[i] + ": a toggle enables Apply and Cancel");
                box.selected = false;
                require(!settings.btn_accept.enabled && settings.btn_cancel.label == "Exit", NAMES[i] + ": toggling back: nothing to apply");
                box.selected = true;
                click(settings.btn_cancel);
                require(!box.selected && !settings.btn_accept.enabled, NAMES[i] + ": Cancel puts the saved state back");
                box.selected = true;
                click(settings.btn_accept);
                require(box.selected && !settings.btn_accept.enabled
                    && !others.some(function(b:*, ...rest):Boolean { return b.selected; }), NAMES[i] + ": Apply saves the toggle, the others unchanged");
                box.selected = false;
                require(settings.btn_accept.enabled, NAMES[i] + ": unchecking a saved on is a change");
                click(settings.btn_cancel);
                require(box.selected, NAMES[i] + ": Cancel: the saved on again");
                root.srp_check_option(NAMES[i], texts[0][LABELS[i]], "0");
                require(!box.selected && !settings.btn_accept.enabled, NAMES[i] + ": the exe's answer sets the saved state");
            }
            trace("SETTINGS a toggle is pending until Apply; Cancel drops it; each box on its own");

            values[10].value = 1;
            content.gotoAndStop(2);
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            require(!boxes.some(function(b:*, ...rest):Boolean { return b.visible; }) && !cross.visible, "not on another tab");
            content.gotoAndStop(1);
            root.set_settings(0, settingsList);
            root.set_values(0, values);
            require(boxes.every(function(b:*, ...rest):Boolean { return b.visible; }) && cross.visible, "back on the gameplay tab");
            require(c._setter_static_show_quests.label == texts[0][5] && !c._setter_static_show_quests.selected
                && crosshair(content).join() == "1,true", "the new tab content: Hide quests and the crosshair list as before");
            var panelIndex:int = content.getChildIndex(content.bottomContainer);
            require(content.getChildIndex(boxes[0]) > panelIndex && content.getChildIndex(cross) > panelIndex,
                "the HUD controls over the bottom panel after a tab change");
            checkLayout(content, boxes, texts, stockOf(content), "after a tab change");
            trace("SETTINGS tabs pass");
            gameplay = [settingsList, values];


            var fontName:String = boxes[0].textField.getTextFormat().font;
            var font:Font = null;
            for each (var fontClass:String in ["CalibriBold", "FontTB", "Calibri", "FontT"])
            {
                if (!domain.hasDefinition(fontClass)) continue;
                var candidate:Font = new (domain.getDefinition(fontClass) as Class)() as Font;
                if (candidate.fontName == fontName) font = candidate;
            }
            if (font == null)
            {
                trace("SETTINGS the label's font " + fontName + " is not a class of the menu: glyphs not checked");
                return;
            }
            var missing:String = "";
            for each (row in texts)
            {
                for each (var text:String in row)
                {
                    for (i = 0; i < text.length; ++i)
                    {
                        var letter:String = text.charAt(i);
                        if (letter != " " && !font.hasGlyphs(letter) && missing.indexOf(letter) < 0) missing += letter;
                    }
                }
            }
            require(missing == "", fontName + " lacks " + missing);
            trace("SETTINGS " + fontName + " has the glyphs of every dictionary's texts");
        }

        // The controls tab as the exe gives it: Invert mouse, Mouse sensitivity (its range), the
        // default button and the three key lists (their rows as applyKeys sizes them).
        private function controls(root:*, settings:*, domain:ApplicationDomain, sensitivity:Number):void
        {
            var c:* = settings.settings_content;
            var given:Array = [];
            frame2(settings);
            root.set_settings(1, [{label: "Invert mouse", setter: setterOf(0, given)},
                {label: "Mouse sensitivity", setter: setterOf(1, given), dataProvider: {minimum: 0.01, maximum: 10, snapInterval: 0.01}},
                {label: "Default settings", setter: setterOf(2, given)}, {label: ""}, {label: ""}, {label: ""}]);
            root.set_values(1, [{id: 0, value: 0}, {id: 1, value: sensitivity}]);
            keys(settings, domain);
        }

        // The controls frame (the tab bar's change calls the exe first, which a test player lacks).
        private function frame2(settings:*):void
        {
            try
            {
                settings.tabs.selectedIndex = 1;
            }
            catch (error:Error)
            {
            }
            if (settings.settings_content.currentFrame != 2) settings.settings_content.gotoAndStop(2);
        }

        // The key lists filled after the tab's bind, as applyKeys fills them: 5 lobby keys, 3 chat keys,
        // 34 game keys (the game's counts).
        private function keys(settings:*, domain:ApplicationDomain):void
        {
            var c:* = settings.settings_content;
            var DP:Class = domain.getDefinition("scaleform.clik.data.DataProvider") as Class;
            var rows:Function = function(n:int, group:int):Array {
                var out:Array = [];
                for (var i:int = 0; i < n; ++i) out.push({action_id: 100 * group + i, group_id: group, type: 1, label: "Action " + i, key: ""});
                return out;
            };
            c.keysLobby.dataProvider = new DP(rows(5, 0));
            c.keysChat.dataProvider = new DP(rows(3, 1));
            c.keysGame.dataProvider = new DP(rows(34, 2));
            c.keysLobby.height = 5 * c.keysLobby.rowHeight;
            c.keysChat.height = 3 * c.keysChat.rowHeight;
        }

        // "Lock vertical sensitivity" and "Vertical sensitivity" (exe patch 112,
        // docs/notes/mouse-pitch.md) on the controls tab: waiting for the exe's answers; ticked (the
        // default), the vertical slider greyed and showing the main one x 0.5625 as it is dragged;
        // unticked, its own from that value, the main one staying when it is dragged; pending until
        // Apply, Cancel putting the saved state back; the exe's answers; the tab's left column (main
        // slider, the box, the vertical slider, Invert mouse, the lobby's keys) and right column (the
        // game's keys, the chat's over the default button) spaced as the tab's own, every
        // dictionary's labels inside the left column.
        private function checkControls(root:*, settings:*, texts:Array, domain:ApplicationDomain):void
        {
            var c:* = settings.settings_content;
            var LOCKED:String = "srp_mouse_vertical_locked";
            // what the greyed slider shows: the main one x 0.5625, to the slider's 0.01
            var shown:Function = function(main:Number):Number { return Math.round(main * 0.5625 / 0.01) * 0.01; };
            controls(root, settings, domain, 1.25);
            var box:* = c.getChildByName("_" + LOCKED);
            var vertical:* = c.getChildByName("_srp_mouse_vertical");
            var main:* = c._slider_mouse_sens;
            require(box != null && vertical != null && !box.visible && !vertical.visible,
                "controls: the box and the vertical slider wait for the exe's answers");
            root.srp_check_option(LOCKED, texts[0][17], "1");
            require(box.visible && !vertical.visible, "controls: the box shown at its answer, the slider waits for its own");
            root.srp_slider_option("srp_mouse_vertical", texts[0][18], "100");
            require(vertical.visible && vertical.alpha < 1 && !vertical.slider.enabled && box.selected
                && box.label == texts[0][17] && vertical.textField.text == texts[0][18],
                "controls: both shown, labelled the exe's; ticked (locked): the vertical slider greyed and inactive");
            require(close(vertical.selectedIndex, 0.7) && close(main.selectedIndex, 1.25) && !settings.btn_accept.enabled,
                "controls: locked, the vertical slider shows the main one x 0.5625 (1.25 -> 0.70), nothing pending: "
                + vertical.selectedIndex);
            require(vertical.slider.minimum == main.slider.minimum && vertical.slider.maximum == main.slider.maximum
                && vertical.slider.snapInterval == main.slider.snapInterval, "controls: the main slider's range");
            // drag the main one: the vertical follows at x 0.5625
            main.slider.value = 2.5;
            require(close(main.selectedIndex, 2.5) && close(vertical.selectedIndex, shown(2.5)), "controls: locked, dragging the "
                + "main slider moves the vertical one with it (2.5 -> " + vertical.selectedIndex + ")");
            main.slider.value = 0.7;
            require(close(vertical.selectedIndex, shown(0.7)), "controls: and again (0.7 -> " + vertical.selectedIndex + ")");
            // untick: its own from the value shown; drag the vertical one: the main stays
            var from:Number = vertical.selectedIndex;
            box.selected = false;
            require(vertical.alpha == 1 && vertical.slider.enabled && close(vertical.selectedIndex, from) && settings.btn_accept.enabled,
                "controls: unticked, the vertical slider active at the value it showed (" + from + "), Apply enabled");
            vertical.slider.value = 3.1;
            require(close(vertical.selectedIndex, 3.1) && close(main.selectedIndex, 0.7), "controls: unlocked, dragging the vertical "
                + "slider leaves the main one: " + main.selectedIndex);
            main.slider.value = 1.5;
            require(close(vertical.selectedIndex, 3.1) && close(main.selectedIndex, 1.5), "controls: unlocked, dragging the main "
                + "slider leaves the vertical one: " + vertical.selectedIndex);
            click(settings.btn_cancel);
            require(box.selected && vertical.alpha < 1 && close(vertical.selectedIndex, shown(main.selectedIndex))
                && !settings.btn_accept.enabled, "controls: Cancel: locked again, following the main one x 0.5625");
            box.selected = false;
            vertical.slider.value = 3.1;
            click(settings.btn_accept);
            require(!box.selected && close(vertical.selectedIndex, 3.1) && !settings.btn_accept.enabled, "controls: Apply keeps it unlocked at 3.1");
            root.srp_check_option(LOCKED, texts[0][17], "0");
            root.srp_slider_option("srp_mouse_vertical", texts[0][18], "310");
            require(!box.selected && close(vertical.selectedIndex, 3.1) && vertical.slider.enabled && !settings.btn_accept.enabled,
                "controls: the exe's answers (unlocked, 310) keep it, nothing pending");
            vertical.slider.value = 4;
            require(settings.btn_accept.enabled, "controls: unlocked, a vertical move is pending");
            vertical.slider.value = 3.1;
            require(!settings.btn_accept.enabled, "controls: and back at the saved value, nothing pending");
            box.selected = true;
            require(vertical.alpha < 1 && !vertical.slider.enabled && close(vertical.selectedIndex, shown(main.selectedIndex))
                && settings.btn_accept.enabled, "controls: ticked, the vertical slider follows the main one x 0.5625 again, greyed, pending");
            click(settings.btn_cancel);
            require(!box.selected && close(vertical.selectedIndex, 3.1) && vertical.slider.enabled && !settings.btn_accept.enabled,
                "controls: Cancel: unlocked at the saved 3.1 again");
            box.selected = true;
            click(settings.btn_accept);
            root.srp_check_option(LOCKED, texts[0][17], "1");
            require(box.selected && close(vertical.selectedIndex, shown(main.selectedIndex)) && !settings.btn_accept.enabled,
                "controls: Apply locked: following the main slider x 0.5625, nothing pending");
            trace("SETTINGS controls: locked follows the main slider x 0.5625, unlocked is its own, pending until Apply, "
                + "Cancel reverts");
            require(!settings.btn_accept.enabled, "controls: nothing pending after the toggles");

            // the layout: a new visit of the frame as the game makes it
            c.gotoAndStop(1);
            controls(root, settings, domain, 1.25);
            root.srp_check_option("srp_mouse_vertical_locked", texts[0][17], "1");
            root.srp_slider_option("srp_mouse_vertical", texts[0][18], "100");
            require(!settings.btn_accept.enabled, "controls: nothing pending at a new visit of the frame");
            var left:Array = [main, box, vertical, c._setter_inverse_mouse, c.s_lobby, c.keysLobby];
            var right:Array = [c.keysGame, c.s_chat_keys, c.keysChat, c.button_default_controls];
            var bottom:Number = Math.max(c.button_default_controls.y + c.button_default_controls.height, c.keysLobby.y + c.keysLobby.height);
            for each (var column:Array in [left, right])
            {
                for (var i:int = 0; i < column.length; ++i)
                {
                    require(column[i].visible, "controls: " + column[i].name + " shown");
                    if (i + 1 < column.length)
                    {
                        // the game's list as sized (it redraws at its next frame)
                        var here:Array = column[i] == c.keysGame ? [c.keysGame.y, c.keysGame.y + c.keysGame.height] : span(column[i]);
                        var next:Array = span(column[i + 1]);
                        require(here[1] <= next[0] + 1, "controls: " + column[i].name + " (" + here + ") over " + column[i + 1].name + " (" + next + ")");
                    }
                }
            }
            require(span(c.keysLobby)[1] <= 400 && span(c.keysChat)[1] <= c.button_default_controls.y,
                "controls: the lobby's keys in the window, the chat's above the default button: " + span(c.keysLobby) + ", " + span(c.keysChat));
            require(near(box.x, c._setter_inverse_mouse.x) && near(vertical.x, main.x) && near(c.keysChat.x, c.keysGame.x),
                "controls: the box at Invert mouse's x, the vertical slider at the main one's, the chat's keys at the game's");
            var edge:Number = 297;                       // the column divider
            for each (var row:Array in texts)
            {
                box.label = row[17];
                box.validateNow();
                require(box.x + box.textField.x + box.textField.textWidth < edge, "controls: " + row[17] + " ends in the left column");
                vertical.label = row[18];
                require(vertical.x + vertical.textField.x + vertical.textField.textWidth < edge, "controls: " + row[18] + " ends in the left column");
            }
            box.label = texts[0][17];
            box.validateNow();
            vertical.label = texts[0][18];
            trace("SETTINGS controls layout: slider " + main.y + ", box " + box.y + ", vertical " + vertical.y + ", invert "
                + c._setter_inverse_mouse.y + ", lobby " + c.s_lobby.y + " to " + span(c.keysLobby)[1] + "; game to " + (c.keysGame.y + c.keysGame.height)
                + ", chat " + c.s_chat_keys.y + " to " + span(c.keysChat)[1] + ", button " + c.button_default_controls.y);
        }

        // The two sliders at their borders (the stock mouse_sensitivity's 0.01 to 10, from its console
        // variable): locked, the greyed slider shows the main one x 0.5625 within its own range; unlocking at
        // either end starts from that value; unlocked, either slider at either end leaves the other; Apply,
        // the exe's answers and a new visit of the tab (Settings reopened) keep each value; after an Apply
        // unlocked, the lock's answer alone shows the vertical value Apply saved (what it gave the exe).
        private function checkBorders(root:*, settings:*, texts:Array, domain:ApplicationDomain):void
        {
            var c:* = settings.settings_content;
            var LOCKED:String = "srp_mouse_vertical_locked";
            var main:* = null;
            // Settings opened again: the exe's settings and answers bound anew (a test player's tab bar
            // cannot change the frame and bind in one go)
            var reopen:Function = function(value:Number, locked:String, vertical:String = null):void {
                controls(root, settings, domain, value);
                root.srp_check_option(LOCKED, texts[0][17], locked);
                if (vertical != null) root.srp_slider_option("srp_mouse_vertical", texts[0][18], vertical);
                main = c._slider_mouse_sens;               // the tab's new instance at each visit
            };
            reopen(1.25, "1", "100");
            var box:* = c.getChildByName("_" + LOCKED), vertical:* = c.getChildByName("_srp_mouse_vertical");
            var least:Number = main.slider.minimum, most:Number = main.slider.maximum;
            var valid:Function = function(v:Number):Boolean { return !isNaN(v) && v >= vertical.slider.minimum - 1e-9 && v <= vertical.slider.maximum + 1e-9; };
            require(close(least, 0.01) && close(most, 10), "borders: the main slider's range 0.01 to 10: " + least + " to " + most);
            for each (var end:Number in [least, most])
            {
                main.slider.value = end;
                var shown:Number = Math.round(end * 0.5625 / 0.01) * 0.01;
                require(box.selected && close(vertical.selectedIndex, shown) && valid(vertical.selectedIndex) && vertical.visible,
                    "borders: locked, the main slider at " + end + ": the greyed one shows " + vertical.selectedIndex + " (" + shown + "), in its range");
                box.selected = false;
                require(close(vertical.selectedIndex, shown) && vertical.slider.enabled && settings.btn_accept.enabled,
                    "borders: unlocked at main " + end + ": it starts from " + vertical.selectedIndex);
                click(settings.btn_accept);
                var saved:String = String(Math.round(shown * 100));
                reopen(end, "0");
                require(!box.selected && close(main.selectedIndex, end) && close(vertical.selectedIndex, shown) && !settings.btn_accept.enabled,
                    "borders: Apply and reopen at main " + end + ": unlocked at the saved " + vertical.selectedIndex + " (" + shown + ")");
                reopen(end, "0", saved);
                require(!box.selected && close(vertical.selectedIndex, shown) && !settings.btn_accept.enabled,
                    "borders: the exe's answer " + saved + " at main " + end + ": unlocked at " + vertical.selectedIndex);
                box.selected = true;
                click(settings.btn_accept);
                reopen(end, "1", saved);
                require(box.selected && close(vertical.selectedIndex, shown), "borders: locked again at main " + end);
            }
            // unlocked: either slider at either end leaves the other
            reopen(1.25, "0", "300");
            for each (end in [least, most])
            {
                vertical.slider.value = end;
                require(close(vertical.selectedIndex, end) && close(main.selectedIndex, 1.25) && valid(vertical.selectedIndex),
                    "borders: unlocked, the vertical slider at " + end + ", the main one stays 1.25: " + main.selectedIndex);
                click(settings.btn_accept);
                reopen(1.25, "0", String(Math.round(end * 100)));
                require(close(vertical.selectedIndex, end) && !box.selected && !settings.btn_accept.enabled,
                    "borders: Apply and reopen at vertical " + end + ": " + vertical.selectedIndex);
            }
            for each (end in [least, most])
            {
                var before:Number = vertical.selectedIndex;
                main.slider.value = end;
                require(close(main.selectedIndex, end) && close(vertical.selectedIndex, before), "borders: unlocked, the main slider at "
                    + end + ", the vertical one stays " + before + ": " + vertical.selectedIndex);
            }
            click(settings.btn_cancel);
            reopen(1.25, "1", "100");
            trace("SETTINGS borders: locked shows main x 0.5625 within range at 0.01 and 10 (0.01, 5.63); unlocking starts there; "
                + "unlocked ends leave the other slider; Apply and reopen keep the values");
        }

        // "Lock vertical sensitivity" and its slider through Apply and Cancel, against patch 112's
        // option store as mouse_pitch.rs keeps it (on_options_call): a call with a value saves it
        // when it is one ("1"/"0"; 1 to 1000 hundredths) and every call answers the saved value at
        // once, in the middle of whatever asked (SrpExe.standIn takes the movie's calls). The stock
        // exe's side as game_options::scaleform_callback has it: accept_changes is options_tab::apply
        // and its initialize_data (root.set_values of the tab), cancel_changes each item's revert
        // (root.set_value). A new visit of the tab is the tab bar's: the frame, the tab bound (its
        // place() asks the exe), then its values. Every path is checked on its own and reported.
        private function checkApplyCancel(root:*, settings:*, texts:Array, domain:ApplicationDomain):void
        {
            var c:* = settings.settings_content;
            var LOCKED:String = "srp_mouse_vertical_locked", VERTICAL:String = "srp_mouse_vertical";
            var exe:Object = {locked: true, vertical: 1.0, main: 1.25};
            var standIn:Function = function(name:String, value:String):void {
                if (name == LOCKED)
                {
                    if (value == "1" || value == "0") exe.locked = value == "1";
                    root.srp_check_option(LOCKED, texts[0][17], exe.locked ? "1" : "0");
                }
                else if (name == VERTICAL)
                {
                    if (value != null && /^[0-9]{1,4}$/.test(value) && int(value) >= 1 && int(value) <= 1000)
                        exe.vertical = int(value) * 0.01;
                    var v:Number = exe.vertical >= 0.01 && exe.vertical <= 10 ? exe.vertical : 0.01;
                    root.srp_slider_option(VERTICAL, texts[0][18], String(Math.min(1000, Math.max(1, Math.floor(v * 100 + 0.5)))));
                }
            };
            var exeClass:Object = domain.getDefinition("SrpExe");
            exeClass.standIn = standIn;
            var box:* = c.getChildByName("_" + LOCKED), vertical:* = c.getChildByName("_" + VERTICAL);
            var main:Function = function():* { return c._slider_mouse_sens; };
            var shown:Function = function(m:Number):Number { return Math.round(m * 0.5625 / 0.01) * 0.01; };
            var visit:Function = function():void {
                c.gotoAndStop(1);
                controls(root, settings, domain, exe.main);
            };
            var apply:Function = function():void {
                require(settings.btn_accept.enabled, "Apply is not enabled");
                click(settings.btn_accept);
                exe.main = main().selectedIndex;
                root.set_values(1, [{id: 0, value: 0}, {id: 1, value: exe.main}]);
            };
            var cancel:Function = function():void {
                click(settings.btn_cancel);
                root.set_value(1, 1, exe.main);
            };
            var start:Function = function(locked:Boolean, v:Number):void {
                if (settings.btn_accept.enabled) cancel();
                settings.visible = true;
                exe.locked = locked;
                exe.vertical = v;
                exe.main = 1.25;
                visit();
            };
            // the tab as the player sees it, and the exe's store
            var state:Function = function():String {
                return (box.selected ? "locked" : "unlocked") + " " + vertical.selectedIndex + (settings.btn_accept.enabled ? ", pending" : "")
                    + (settings.btn_cancel.label == exit ? ", Exit" : ", Cancel") + "; exe " + (exe.locked ? "locked" : "unlocked") + " " + Math.round(exe.vertical * 100) / 100;
            };
            var expect:Function = function(what:String, locked:Boolean, v:Number, pending:Boolean, exeLocked:Boolean, exeV:Number):void {
                var ok:Boolean = box.selected == locked && close(vertical.selectedIndex, v) && settings.btn_accept.enabled == pending
                    && (settings.btn_cancel.label != exit) == pending && vertical.slider.enabled == !locked
                    && exe.locked == exeLocked && Math.abs(exe.vertical - exeV) < 1e-6;
                require(ok, what + ": " + state() + ", not " + (locked ? "locked" : "unlocked") + " " + v + (pending ? ", pending" : "")
                    + "; exe " + (exeLocked ? "locked" : "unlocked") + " " + exeV);
            };
            var exit:String = settings.btn_cancel.label;   // nothing pending here (checkBorders ends so)
            var failed:Array = [];
            var cases:Array = [
                ["first visit, locked", function():void {
                    start(true, 1);
                    expect("shown", true, shown(1.25), false, true, 1);
                }],
                ["toggle the lock, Cancel", function():void {
                    start(true, 1);
                    box.selected = false;
                    expect("unticked", false, shown(1.25), true, true, 1);
                    cancel();
                    expect("Cancel", true, shown(1.25), false, true, 1);
                    visit();
                    expect("a new visit", true, shown(1.25), false, true, 1);
                }],
                ["toggle the lock, Apply", function():void {
                    start(true, 1);
                    box.selected = false;
                    apply();
                    expect("Apply", false, shown(1.25), false, false, shown(1.25));
                    visit();
                    expect("a new visit", false, shown(1.25), false, false, shown(1.25));
                    box.selected = true;
                    expect("the lock again", true, shown(1.25), true, false, shown(1.25));
                }],
                ["unlock, move the vertical slider, Apply", function():void {
                    start(true, 1);
                    box.selected = false;
                    vertical.slider.value = 2;
                    expect("moved", false, 2, true, true, 1);
                    apply();
                    expect("Apply", false, 2, false, false, 2);
                    visit();
                    expect("a new visit", false, 2, false, false, 2);
                    box.selected = true;
                    expect("then the lock", true, shown(1.25), true, false, 2);
                }],
                ["unlocked, move the vertical slider, Cancel", function():void {
                    start(false, 3.1);
                    expect("shown", false, 3.1, false, false, 3.1);
                    vertical.slider.value = 4;
                    expect("moved", false, 4, true, false, 3.1);
                    cancel();
                    expect("Cancel", false, 3.1, false, false, 3.1);
                    visit();
                    expect("a new visit", false, 3.1, false, false, 3.1);
                }],
                ["unlocked, move the vertical slider, Apply", function():void {
                    start(false, 3.1);
                    vertical.slider.value = 4;
                    apply();
                    expect("Apply", false, 4, false, false, 4);
                    visit();
                    expect("a new visit", false, 4, false, false, 4);
                }],
                ["locked, move the main slider, Cancel", function():void {
                    start(true, 1);
                    main().slider.value = 2.5;
                    expect("moved", true, shown(2.5), true, true, 1);
                    cancel();
                    require(close(main().selectedIndex, 1.25), "Cancel: the main slider back at 1.25: " + main().selectedIndex);
                    expect("Cancel", true, shown(1.25), false, true, 1);
                }],
                ["locked, move the main slider, Apply", function():void {
                    start(true, 1);
                    main().slider.value = 2.5;
                    apply();
                    expect("Apply", true, shown(2.5), false, true, 1);
                    visit();
                    expect("a new visit", true, shown(2.5), false, true, 1);
                }],
                ["toggle the lock twice, locked", function():void {
                    start(true, 1);
                    box.selected = false;
                    box.selected = true;
                    expect("back", true, shown(1.25), false, true, 1);
                }],
                ["toggle the lock twice, unlocked", function():void {
                    start(false, 3.1);
                    box.selected = true;
                    expect("ticked", true, shown(1.25), true, false, 3.1);
                    box.selected = false;
                    expect("back", false, 3.1, false, false, 3.1);
                }],
                ["unlock, move the vertical slider, lock again", function():void {
                    start(true, 1);
                    box.selected = false;
                    vertical.slider.value = 4;
                    box.selected = true;
                    expect("locked again: nothing to apply", true, shown(1.25), false, true, 1);
                }],
                ["Cancel with nothing pending is Exit", function():void {
                    start(true, 1);
                    click(settings.btn_cancel);
                    require(!settings.visible, "Exit: Settings closed");
                    root.show_settings(true);
                    expect("opened again", true, shown(1.25), false, true, 1);
                }]];
            for each (var entry:Array in cases)
            {
                try
                {
                    entry[1]();
                    trace("SETTINGS apply-cancel " + entry[0] + ": pass");
                }
                catch (error:Error)
                {
                    failed.push(entry[0]);
                    trace("SETTINGS apply-cancel " + entry[0] + ": FAIL " + error.message);
                }
            }
            if (settings.btn_accept.enabled) cancel();
            settings.visible = true;
            exeClass.standIn = null;
            require(failed.length == 0, "apply-cancel: " + failed.join("; "));
        }

        // The exe's setter of option `i`: records [i, value] and gives the value back.
        private static function setterOf(i:int, given:Array):Function
        {
            return function(value:*):* {
                given.push([i, value]);
                return value;
            };
        }
    }
}
