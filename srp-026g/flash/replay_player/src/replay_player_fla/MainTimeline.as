package replay_player_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.NumericStepper;
   import scaleform.clik.controls.Slider;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import scaleform.clik.events.SliderEvent;
   import scaleform.clik.layout.Layout;
   import scaleform.clik.layout.LayoutData;
   import scaleform.gfx.Extensions;
   import survarium.constants.TeamSide;
   import survarium.replay.MarkerTicks;
   import survarium.replay.ReplayText;
   import survarium.replay.ViewerControls;
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var playersBar:ButtonBarContext;
      
      public var freeViewButton:SvButton1;
      
      public var labelsBar:ButtonBar;
      
      public var bottomPanel:MovieClip;
      
      public var slider:Slider;
      
      public var btn_start:Button;
      
      public var btn_end:Button;
      
      public var btn_play:Button;
      
      public var stepper:NumericStepper;
      
      public var btn_previous_label:Button;
      
      public var btn_previous_frame:Button;
      
      public var btn_next_label:Button;
      
      public var btn_next_frame:Button;
      
      public var btn_camera:Button;
      
      public var _ignoreEvent:Boolean;
      
      public var offset:int;
      
      public var optionsContainer:MovieClip;
      
      public var layout:Layout;
      
      // The scale the controls are drawn at (onStageResize).
      public var scaleFactor:Number = 1;
      
      public var temp_data:Object;
      
      public var temp_players:Array;
      
      public var i:uint;
      
      public var temp_options:Array;
      
      public var inDrag:Boolean;

      // srp's: the markers as ticks on the bar (made at the first use: markerTicks())
      public var _ticks:MarkerTicks;

      // srp's: a replay viewer's Back, Moments switch and key hint (start_viewer), null for a .sur's
      public var viewer:ViewerControls;

      // srp's: the playback's speed in percent (set_replay_speed, or the stepper's own steps).
      public var speedPercent:int = 100;
      
      // The seek keys' steps, in the seek bar's unit (ms): an arrow, A or D
      // a few seconds, the comma and the full stop one tick of the game.
      public static const SEEK_KEY_MS:int = 5000;
      
      public static const SEEK_TICK_MS:int = 50;
      
      // The free view flies with A and D, so there they do not seek.
      public var freeView:Boolean;
      
      // The player followed from the list: every place they had and the view
      // asked for (null: none was clicked, or the free view's button was).
      public var followed:Object;
      
      // The places in the match, as the game last told (srp_places).
      public var placesIn:Array = [];
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_labelsBar_();
         this.__setProp_playersBar_();
         this.__setProp_freeViewButton_();
      }
      
      public function set_camera_smooth(value:Boolean) : void
      {
         this.btn_camera.selected = value;
      }
      
      public function onCameraChange(e:Event) : void
      {
         trace("camera_smooth_changed",this.btn_camera);
         ExternalInterface.call("camera_smooth_changed",this.btn_camera.selected);
      }
      
      public function onFreeViewClicked(e:Event) : void
      {
         this.playersBar.selectedIndex = -1;
         freeView = true;
         followed = null;
         ExternalInterface.call("select_free_view");
      }
      
      public function onPlayerClikedClicked(e:Event) : void
      {
         if(this._ignoreEvent)
         {
            _ignoreEvent = false;
            return;
         }
         var innerButton:Boolean = e.target.parent is Button;
         trace("onPlayerClikedClicked",e.target,innerButton);
         var buttonRenderer:Button = e.target.parent is Button ? e.target.parent as Button : e.target as Button;
         if(buttonRenderer)
         {
            freeView = false;
            trace("PlayerView",buttonRenderer.data.name,innerButton ? (e.target.name == "btnFirstView" ? 0 : 1) : 0);
            // Every place the player had: the game follows the one that is in
            // the match now and does nothing for the others.
            followed = {
               "ids":buttonRenderer.data.ids,
               "view":innerButton ? (e.target.name == "btnFirstView" ? 0 : 1) : 0
            };
            for each(var place:uint in followed.ids)
            {
               ExternalInterface.call("select_player",place,followed.view);
            }
         }
         if(innerButton)
         {
            _ignoreEvent = true;
         }
         trace("addEventListener");
         stage.addEventListener(Event.ENTER_FRAME,this.onEventRender,true);
      }
      
      public function onEventRender(e:Event) : void
      {
         trace("onEventRender");
         _ignoreEvent = false;
         stage.removeEventListener(Event.ENTER_FRAME,this.onEventRender,true);
      }
      
      // The lobby and the HUD scale themselves from 1920 x 1080 (by the width up to
      // an aspect of 1.85, by the height above); the replay's controls were drawn
      // 1:1, half their size at 4K. They now take that scale, never below 1: at
      // 1080p and below they are as they were. The layout anchors them to the
      // scaled stage's edges.
      public function onStageResize(e:Event = null) : void
      {
         var w:Number = stage.stageWidth;
         var h:Number = stage.stageHeight;
         var factor:Number = w / h < 1.85 ? w / 1920 : h / 1080;
         if(!(factor > 1))
         {
            factor = 1;
         }
         scaleFactor = factor;
         scaleX = scaleY = factor;
         this.layout.setScaleFactor(factor);
         this.layout.reflow();
         this.freeViewButton.x = stage.stageWidth / this.scaleFactor - this.freeViewButton.width - 10;
         this.freeViewButton.y = this.playersBar.y + this.playersBar.height + 4;
         this.markerTicks().draw();
      }

      // srp's: the bar's ticks, one a marker (survarium.replay.MarkerTicks), in the font of the controls.
      public function markerTicks() : MarkerTicks
      {
         if(this._ticks == null)
         {
            this._ticks = new MarkerTicks(this.bottomPanel.slider,this.freeViewButton.textField.getTextFormat().font);
         }
         return this._ticks;
      }
      
      // srp's: the speeds the exe plays (Speed::ALL in replay-control), in percent, slowest first.
      public static const SPEEDS:Array = [25,50,100,200,400,800];

      // A press of the stepper: the next speed of SPEEDS from the shown one, the way the value moved
      // (the value itself when it is one, as Home and End set), shown and sent; none past the ends.
      public function onStepperUpdate(e:Event) : void
      {
         var asked:int = Math.round(this.stepper.value * 100);
         var from:int = this.speedPercent;
         var to:int = from;
         if(SPEEDS.indexOf(asked) >= 0)
         {
            to = asked;
         }
         else
         {
            for each(var s:int in SPEEDS)
            {
               if(asked > from ? s > from && to == from : s < from)
               {
                  to = s;
               }
            }
         }
         trace("onStepperUpdate::",asked,to);
         this.speedPercent = to;
         this.showSpeed();
         if(to != from)
         {
            ExternalInterface.call("update_play_speed",to / 100);
         }
      }
      
      public function onPlayClicked(e:Event) : void
      {
         trace("onPlayClicked::",!this.btn_play.selected);
         ExternalInterface.call("button_play_clicked",!this.btn_play.selected);
      }
      
      public function onLabelChange(e:Event) : void
      {
         this.labelsBar.selectedIndex += e.target == this.btn_previous_label ? -1 : 1;
      }
      
      public function onFrameChange(e:Event) : void
      {
         this.slider.value += e.target == this.btn_previous_frame ? -1 : 1;
      }
      
      // The keys of a replay, as a video player's (the owner, 2026-11-10). The
      // movie gets no key from the game: exe patch 140 names each one here as
      // it goes down. Left and right (and A and D while a player is followed:
      // the free view flies with them) 5 s back and on, the comma and the
      // full stop one tick, through the seek bar as the frame buttons go; the
      // space bar plays and pauses as the play button does.
      public function srp_key(name:String) : void
      {
         var step:int = 0;
         switch(name)
         {
            case "followed":
               freeView = false;
               return;
            case "free_view":
               freeView = true;
               return;
            case "another":
               // The game's next or previous player (F6, F5): the row
               // clicked is not the one watched any more, and the list
               // cannot tell which is.
               followed = null;
               return;
            case "pause":
               this.btn_play.selected = !this.btn_play.selected;
               this.onPlayClicked(null);
               return;
            case "back":
               step = -SEEK_KEY_MS;
               break;
            case "forward":
               step = SEEK_KEY_MS;
               break;
            case "strafe_back":
               step = this.freeView ? 0 : -SEEK_KEY_MS;
               break;
            case "strafe_forward":
               step = this.freeView ? 0 : SEEK_KEY_MS;
               break;
            case "tick_back":
               step = -SEEK_TICK_MS;
               break;
            case "tick_forward":
               step = SEEK_TICK_MS;
               break;
         }
         var to:Number = Math.max(this.slider.minimum,Math.min(this.slider.maximum,this.slider.position + step));
         if(step != 0 && to != this.slider.position)
         {
            this.slider.value = to;
         }
      }
      
      // The places in the match, a Boolean a place, from exe patch 140 when
      // they change (an entry, a leave, a seek). The game follows a place only
      // from select_player, and only one in the match at that moment, so the
      // followed player's place that comes in (the same one again, or their
      // next) is asked for again (the owner, 2026-11-11: "if the person
      // leaves, their replay just stops even if they enter again"). Not in
      // the free view, which the viewer chose.
      public function srp_places(... places) : void
      {
         var before:Array = this.placesIn;
         this.placesIn = places;
         if(!this.followed || this.freeView)
         {
            return;
         }
         for each(var place:uint in this.followed.ids)
         {
            if(places[place] && !before[place])
            {
               ExternalInterface.call("select_player",place,this.followed.view);
            }
         }
      }
      
      public function onMoveClicked(e:Event) : void
      {
         this.slider.value = e.target == this.btn_start ? this.slider.minimum : this.slider.maximum;
      }
      
      public function onSliderUpdate(e:Event) : void
      {
         var value:uint = this.slider.position;
         trace("update_position::",value);
         ExternalInterface.call("update_position",value);
         this.checkFrameButtons();
         this.selectActiveLabel();
      }
      
      // srp's: the playback's speed from the exe (each Playback note), in percent, 25 to 800 (outside:
      // the nearest end). The stepper shows it as 0.25x, 0.5x, 1x, 2x, 4x, 8x; nothing goes back to the
      // exe (update_play_speed is the stepper's own steps, SPEEDS).
      public function set_replay_speed(percent:int) : void
      {
         this.speedPercent = Math.max(25,Math.min(800,percent));
         this.showSpeed();
      }

      // The stepper's value and label from speedPercent, sending nothing. The bottom panel's frames
      // set the stepper's art values again (bottom_staff_1): shown again after each (show_buttons),
      // and the stepper's steps go to the exe again from then.
      public function showSpeed() : void
      {
         this.stepper.removeEventListener(IndexEvent.INDEX_CHANGE,this.onStepperUpdate);
         this.stepper.labelFunction = this.speedLabel;
         // the ends are SPEEDS' (their arrows off there); a step of 0.01 keeps each speed whole and
         // only says the way a press moved (onStepperUpdate)
         this.stepper.maximum = SPEEDS[SPEEDS.length - 1] / 100;
         this.stepper.minimum = SPEEDS[0] / 100;
         this.stepper.stepSize = 0.01;
         this.stepper.value = this.speedPercent / 100;
         this.stepper.validateNow();
         // its own updateLabel leaves a field with a selection as it is
         this.stepper.textField.text = this.speedLabel(this.stepper.value);
         this.stepper.addEventListener(IndexEvent.INDEX_CHANGE,this.onStepperUpdate);
      }

      // A speed as the stepper shows it: 0.25x, 1x, 8x.
      public function speedLabel(value:Number) : String
      {
         return String(Math.round(value * 100) / 100) + "x";
      }

      public function onShowButtonsFrame(e:Event) : void
      {
         removeEventListener(Event.EXIT_FRAME,this.onShowButtonsFrame);
         this.showSpeed();
      }

      public function setData(value:Object) : void
      {
         trace("setData");
         this.slider.minimum = 0;
         this.slider.snapInterval = 1;
         if(value.match_time != undefined)
         {
            this.slider.maximum = value.match_time;
         }
         this.setPosition(0);
         this.speedPercent = 100;
         this.showSpeed();
         this.layout.reflow();
         this.markerTicks().draw();
      }
      
      public function roundValue(value:Number) : Number
      {
         var newValue:Number = Math.round(value / 0.01) * 0.01;
         trace("newV::",value,newValue);
         return newValue;
      }
      
      public function setPosition(value:int) : void
      {
         this.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onSliderUpdate);
         this.slider.value = value;
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onSliderUpdate);
         this.selectActiveLabel();
         this.checkFrameButtons();
      }
      
      public function selectActiveLabel() : void
      {
         if(this.labelsBar.dataProvider == null)
         {
            return;
         }
         var labels:DataProvider = this.labelsBar.dataProvider as DataProvider;
         var l:int = labels.length - 1;
         for(var i:int = l; i >= 0; i--)
         {
            if(this.slider.position >= labels[i].time)
            {
               if(this.labelsBar.selectedIndex != i)
               {
                  this.labelsBar.removeEventListener(IndexEvent.INDEX_CHANGE,this.updateActiveLabel);
                  this.setActiveLabel(i);
                  this.labelsBar.addEventListener(IndexEvent.INDEX_CHANGE,this.updateActiveLabel);
               }
               return;
            }
         }
         this.setActiveLabel(-1);
      }
      
      public function updateActiveLabel(e:Event) : void
      {
         var activeLabel:uint = (this.labelsBar.selectedIndex);
         trace("updateActiveLabel::",activeLabel);
         this.slider.value = this.labelsBar.dataProvider[activeLabel].time;
         this.checkLabelButtons();
      }
      
      public function checkFrameButtons() : void
      {
         this.btn_start.enabled = this.btn_previous_frame.enabled = this.slider.position != 0;
         this.btn_end.enabled = this.btn_next_frame.enabled = this.slider.position != this.slider.maximum;
      }
      
      public function checkLabelButtons() : void
      {
         this.btn_previous_label.enabled = this.labelsBar.dataProvider != null && this.labelsBar.selectedIndex != -1 && this.labelsBar.selectedIndex != 0 && this.labelsBar.dataProvider.length != 0;
         this.btn_next_label.enabled = this.labelsBar.dataProvider != null && this.labelsBar.dataProvider.length - 1 != this.labelsBar.selectedIndex;
      }
      
      public function setActiveLabel(value:int) : void
      {
         this.labelsBar.removeEventListener(IndexEvent.INDEX_CHANGE,this.updateActiveLabel);
         this.labelsBar.selectedIndex = value;
         this.labelsBar.validateNow();
         this.checkLabelButtons();
         this.labelsBar.addEventListener(IndexEvent.INDEX_CHANGE,this.updateActiveLabel);
      }
      
      public function beginDrag(e:Event) : void
      {
         inDrag = true;
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onEndDrag);
      }
      
      public function onEndDrag(e:Event) : void
      {
         trace("onED");
         inDrag = false;
         this.slider.removeEventListener(SliderEvent.VALUE_CHANGE,this.onEndDrag);
      }
      
      public function updateTest(e:Event) : void
      {
         if(e.target != stage)
         {
            return;
         }
         var showBtns:Boolean = this.bottomPanel.currentFrame == 2;
         trace("showBtns",showBtns);
         this.show_buttons(showBtns);
         this.set_players_list(this.temp_players);
      }
      
      public function updatePos(e:Event) : void
      {
         this.set_position(this.slider.position + 1);
         if(this.slider.position > 100)
         {
            this.set_play_state(false);
         }
      }
      
      public function set_match_time(value:uint) : void
      {
         this.slider.maximum = value;
         this.markerTicks().draw();
      }
      
      public function set_play_state(value:Boolean) : void
      {
         this.btn_play.selected = value;
      }
      
      public function set_active_label(value:int) : void
      {
         this.setActiveLabel(value);
      }
      
      public function set_data(value:Object) : void
      {
         this.setData(value);
      }
      
      public function set_position(value:uint) : void
      {
         trace("setPos::",this.inDrag);
         if(this.inDrag)
         {
            return;
         }
         this.setPosition(value);
      }
      
      public function show_buttons(value:Boolean) : void
      {
         // the frame's script sets the stepper's art values (0): not a speed of the player's
         this.stepper.removeEventListener(IndexEvent.INDEX_CHANGE,this.onStepperUpdate);
         this.bottomPanel.gotoAndStop(value ? 1 : 2);
         this.checkLabelButtons();
         this.checkFrameButtons();
         this.bottomPanel.layoutData.offsetV = value ? -10 : 25;
         this.layout.reflow();
         // the speed over them once the frame's script has run
         addEventListener(Event.EXIT_FRAME,this.onShowButtonsFrame);
      }
      
      // The markers: ticks on the bar (srp's, MarkerTicks), and the list the previous/next label buttons
      // step through, kept hidden (the ticks show what it showed, at their place in time).
      public function set_markers_list(value:Array) : void
      {
         var labels:Array = value;
         this.labelsBar.height = 25 * labels.length + 2;
         this.labelsBar.dataProvider = new DataProvider(labels);
         this.labelsBar.visible = false;
         this.layout.reflow();
         this.selectActiveLabel();
         this.markerTicks().draw(labels);
      }
      
      // srp's: the markers as one string from the exe, `time_ms:kind:whose:actor:victim;...`, their
      // labels from set_marker_names' list (ReplayText.markers).
      public function set_markers_text(value:String) : void
      {
         this.set_markers_list(ReplayText.markers(value,this.markerNames));
      }

      private var markerNames:Array = [];

      // srp's: the names by place the markers show, `place:name;...` (ReplayText.names), told before
      // set_markers_text.
      public function set_marker_names(value:String) : void
      {
         this.markerNames = ReplayText.names(value);
      }

      // srp's: the controls a replay viewer's (the exe, once): its Back button, Moments switch and key
      // hint (ViewerControls), with their texts in the running language: Back, the switch's label and
      // its choices All, Mine, Followed, the hint's actions Cursor, Pause, Filter, Hide and its Ctrl and
      // Space caps, and the markers' kinds Kill, Objective, Death and another event (MarkerTicks).
      public function start_viewer(back:String, moments:String, all:String, mine:String, followed:String, cursor:String, pause:String, filter:String, hide:String, ctrl:String, space:String, kill:String, objective:String, death:String, event:String) : void
      {
         MarkerTicks.setKindWords([kill,objective,death,event]);
         if(this.viewer != null)
         {
            return;
         }
         this.viewer = new ViewerControls(this.bottomPanel,this.freeViewButton.textField.getTextFormat().font,[back,moments,all,mine,followed,cursor,pause,filter,hide,ctrl,space]);
         addChild(this.viewer);
         this.viewer.place();
      }

      // srp's: the Moments switch's choice (0 All, 1 Mine, 2 Followed; Mine offered when `mine` is not
      // 0), shown on the switch and by the ticks (the others faded).
      public function set_moments(choice:int, mine:int) : void
      {
         this.markerTicks().moments = choice;
         if(this.viewer != null)
         {
            this.viewer.setMoments(choice,mine != 0);
         }
      }

      // srp's: the hud's cursor shown (Ctrl): the key hint shows with it.
      public function set_cursor_shown(shown:Boolean) : void
      {
         if(this.viewer != null)
         {
            this.viewer.setCursor(shown);
         }
      }

      // srp's: the players as one string from the exe, `place:team:name;...` (ReplayText.players).
      public function set_players_text(value:String) : void
      {
         this.set_players_list(ReplayText.players(value));
      }

      public function set_players_list(value:Array) : void
      {
         trace("set_players_list::",value ? value.length : null);
         if(!value)
         {
            this.freeViewButton.label = "no_players_list";
            return;
         }
         // The game gives a row for every roster place, and a player who left
         // and came back has a place for each time (the owner, 2026-11-10:
         // "why player list duplicated many players", "only one of those two is
         // clickable"): one row a player of a team, with all its places.
         var players:Array = [];
         var rows:Object = {};
         for each(var player:Object in value)
         {
            var key:String = player.team + "/" + player.name;
            if(rows[key] == undefined)
            {
               rows[key] = player;
               player.ids = [];
               players.push(player);
            }
            rows[key].ids.push(uint(player.id));
         }
         players.sortOn("team");
         this.playersBar.height = 25 * players.length + 2;
         this.playersBar.dataProvider = new DataProvider(players);
         this.playersBar.visible = players.length > 0;
         this.layout.reflow();
         this.freeViewButton.x = stage.stageWidth / this.scaleFactor - this.freeViewButton.width - 10;
         // under the last player (it was over it), as onStageResize places it
         this.freeViewButton.y = this.playersBar.y + this.playersBar.height + 4;
      }
      
      public function show_scene_options(value:Boolean) : void
      {
         this.optionsContainer.visible = value;
      }
      
      public function onSliderCheckBoxClick(value:ButtonEvent) : void
      {
         if(value.target.name == "checkBox")
         {
            trace("sliderCheckBox",value.target,value.currentTarget);
            ExternalInterface.call("scene_setting_checked",uint(value.currentTarget.id),Boolean(value.target.selected));
         }
      }
      
      public function onSliderUpdateOptions(value:IndexEvent) : void
      {
         trace("sliderUpdated",value.target.value);
         ExternalInterface.call("scene_setting_updated",uint(value.currentTarget.id),Number(value.target.value));
      }
      
      public function set_scene_settings(value:Array) : void
      {
         if(this.optionsContainer.numChildren)
         {
            while(this.optionsContainer.numChildren)
            {
               this.optionsContainer.removeChildAt(0);
            }
         }
         for(var i:uint = 0; i < value.length; i++)
         {
            var slider:SrvOptionSlider;
            slider = new SrvOptionSlider();
            slider.y = i * (slider.height + 1);
            if(value[i].snapInterval == undefined)
            {
               value[i].snapInterval = 0.01;
            }
            slider.id = value[i].id;
            slider.addEventListener(IndexEvent.INDEX_CHANGE,this.onSliderUpdateOptions);
            slider.addEventListener(ButtonEvent.CLICK,this.onSliderCheckBoxClick);
            slider.label = value[i].label;
            slider.dataProvider = value[i];
            slider.editable = true;
            slider.slider.value = value[i].value;
            slider.checkBox.visible = value[i].show_checkbox;
            this.optionsContainer.addChild(slider);
         }
         this.layout.reflow();
      }
      
      internal function __setProp_labelsBar_() : *
      {
         try
         {
            this.labelsBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.labelsBar.autoSize = "none";
         this.labelsBar.buttonWidth = 60;
         this.labelsBar.direction = "vertical";
         this.labelsBar.enabled = true;
         this.labelsBar.enableInitCallback = false;
         this.labelsBar.focusable = true;
         this.labelsBar.itemRendererName = "SvButton6";
         this.labelsBar.spacing = 0;
         this.labelsBar.visible = true;
         try
         {
            this.labelsBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_playersBar_() : *
      {
         try
         {
            this.playersBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.playersBar.autoSize = "none";
         this.playersBar.buttonWidth = 120;
         this.playersBar.direction = "vertical";
         this.playersBar.enabled = true;
         this.playersBar.enableInitCallback = false;
         this.playersBar.focusable = true;
         this.playersBar.itemRendererName = "SvButtonPlayerRenderer";
         this.playersBar.spacing = 0;
         this.playersBar.visible = true;
         try
         {
            this.playersBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_freeViewButton_() : *
      {
         try
         {
            this.freeViewButton["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.freeViewButton.autoRepeat = false;
         this.freeViewButton.autoSize = "none";
         this.freeViewButton.data = "";
         this.freeViewButton.enabled = true;
         this.freeViewButton.enableInitCallback = false;
         this.freeViewButton.focusable = true;
         this.freeViewButton.label = "free view";
         this.freeViewButton.selected = false;
         this.freeViewButton.toggle = false;
         this.freeViewButton.visible = true;
         try
         {
            this.freeViewButton["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
         this.labelsBar.height = 1;
         this.labelsBar.visible = false;
         slider = this.bottomPanel.slider;
         btn_start = this.bottomPanel.btn_start;
         btn_end = this.bottomPanel.btn_end;
         btn_play = this.bottomPanel.btn_play;
         stepper = this.bottomPanel.speedStepper;
         btn_previous_label = this.bottomPanel.btn_previous_label;
         btn_previous_frame = this.bottomPanel.btn_previous_frame;
         btn_next_label = this.bottomPanel.btn_next_label;
         btn_next_frame = this.bottomPanel.btn_next_frame;
         btn_camera = this.bottomPanel.btn_camera;
         trace("btn_camera");
         this.btn_camera.addEventListener(ButtonEvent.CLICK,this.onCameraChange);
         this.btn_previous_label.addEventListener(ButtonEvent.CLICK,this.onLabelChange);
         this.btn_next_label.addEventListener(ButtonEvent.CLICK,this.onLabelChange);
         this.btn_previous_frame.addEventListener(ButtonEvent.CLICK,this.onFrameChange);
         this.btn_next_frame.addEventListener(ButtonEvent.CLICK,this.onFrameChange);
         this.btn_start.addEventListener(ButtonEvent.CLICK,this.onMoveClicked);
         this.btn_end.addEventListener(ButtonEvent.CLICK,this.onMoveClicked);
         this.playersBar.addEventListener(ButtonEvent.CLICK,this.onPlayerClikedClicked);
         this.freeViewButton.addEventListener(ButtonEvent.CLICK,this.onFreeViewClicked);
         _ignoreEvent = false;
         offset = 10;
         this.bottomPanel.layoutData = new LayoutData();
         this.bottomPanel.layoutData.alignV = "bottom";
         this.bottomPanel.layoutData.alignH = "center";
         this.bottomPanel.layoutData.offsetH = 0;
         this.bottomPanel.layoutData.offsetV = -this.offset;
         this.labelsBar.layoutData = new LayoutData();
         this.labelsBar.layoutData.alignV = "center";
         this.labelsBar.layoutData.alignH = "left";
         this.labelsBar.layoutData.offsetH = this.offset;
         this.labelsBar.layoutData.offsetV = -25;
         this.playersBar.layoutData = new LayoutData();
         this.playersBar.layoutData.alignV = "center";
         this.playersBar.layoutData.alignH = "right";
         this.playersBar.layoutData.offsetH = -this.offset;
         this.playersBar.layoutData.offsetV = -25;
         optionsContainer = new MovieClip();
         this.optionsContainer.layoutData = new LayoutData();
         this.optionsContainer.layoutData.alignV = "bottom";
         this.optionsContainer.layoutData.alignH = "left";
         this.optionsContainer.layoutData.offsetH = this.offset;
         this.optionsContainer.layoutData.offsetV = -25;
         addChild(this.optionsContainer);
         layout = new Layout();
         this.layout.tiedToStageSize = true;
         addChild(this.layout);
         stage.addEventListener(Event.RESIZE,this.onStageResize);
         this.onStageResize();
         this.btn_play.selected = true;
         this.stepper.labelFunction = this.speedLabel;
         this.stepper.addEventListener(IndexEvent.INDEX_CHANGE,this.onStepperUpdate);
         this.btn_play.addEventListener(ButtonEvent.CLICK,this.onPlayClicked);
         this.slider.addEventListener(SliderEvent.VALUE_CHANGE,this.onSliderUpdate);
         this.labelsBar.visible = false;
         this.labelsBar.addEventListener(IndexEvent.INDEX_CHANGE,this.updateActiveLabel);
         temp_data = {
            "min_replay_speed":0.20111111,
            "max_replay_speed":1.999995,
            "replay_speed_step_size":0.092,
            "match_time":1000,
            "labels":[{
               "label":"match_strt",
               "time":200
            },{
               "label":"0:1",
               "time":500
            },{
               "label":"0:2",
               "time":750
            }]
         };
         temp_players = [];
         for(i = 0; this.i < 8; ++this.i)
         {
            this.temp_players.push({
               "id":this.i,
               "name":"Player" + this.i,
               "team":(this.i < 4 ? TeamSide.ALLY : TeamSide.ENEMY)
            });
         }
         temp_options = [];
         for(i = 0; this.i < 6; ++this.i)
         {
            this.temp_options.push({
               "id":this.i,
               "value":this.i,
               "minimum":0,
               "maximum":10,
               "show_checkbox":true,
               "selected":true,
               "label":"habahaba" + this.i.toString()
            });
         }
         if(Extensions.isGFxPlayer)
         {
            this.set_scene_settings(this.temp_options);
            this.show_scene_options(true);
            this.setData(this.temp_data);
            this.set_camera_smooth(true);
         }
         this.slider.liveDragging = false;
         this.slider.thumb.addEventListener(MouseEvent.MOUSE_DOWN,this.beginDrag,false,0,true);
         inDrag = false;
      }
   }
}

