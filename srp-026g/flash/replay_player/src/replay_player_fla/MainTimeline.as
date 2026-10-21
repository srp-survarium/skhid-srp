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
      
      public var temp_data:Object;
      
      public var temp_players:Array;
      
      public var i:uint;
      
      public var temp_options:Array;
      
      public var inDrag:Boolean;
      
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
            trace("PlayerView",buttonRenderer.data.name,innerButton ? (e.target.name == "btnFirstView" ? 0 : 1) : 0);
            ExternalInterface.call("select_player",uint(buttonRenderer.data.id),uint(innerButton ? (e.target.name == "btnFirstView" ? 0 : 1) : 0));
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
      
      public function onStageResize(e:Event) : void
      {
         this.layout.reflow();
         this.freeViewButton.x = stage.stageWidth - this.freeViewButton.width - 10;
         this.freeViewButton.y = this.playersBar.y + this.playersBar.height + 4;
      }
      
      public function onStepperUpdate(e:Event) : void
      {
         var value:Number = this.stepper.value;
         trace("onStepperUpdate::",value);
         ExternalInterface.call("update_play_speed",value);
         this.updateSpeedStepper();
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
         this.stepper.stepSize = 0.1;
         this.stepper.minimum = 0.01;
         this.stepper.maximum = 10;
         this.stepper.value = 1;
         this.layout.reflow();
      }
      
      public function updateSpeedStepper() : void
      {
         if(this.stepper.value <= 0.1)
         {
            if(this.stepper.stepSize != 0.01)
            {
               this.stepper.stepSize = 0.01;
            }
         }
         else if(this.stepper.stepSize != 0.1)
         {
            this.stepper.stepSize = 0.1;
         }
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
         this.bottomPanel.gotoAndStop(value ? 1 : 2);
         this.checkLabelButtons();
         this.checkFrameButtons();
         this.bottomPanel.layoutData.offsetV = value ? -10 : 25;
         this.layout.reflow();
      }
      
      public function set_markers_list(value:Array) : void
      {
         var labels:Array = value;
         this.labelsBar.height = 25 * labels.length + 2;
         this.labelsBar.dataProvider = new DataProvider(labels);
         this.labelsBar.visible = labels.length > 0;
         this.layout.reflow();
         this.selectActiveLabel();
      }
      
      public function set_players_list(value:Array) : void
      {
         trace("set_players_list::",value ? value.length : null);
         if(!value)
         {
            this.freeViewButton.label = "no_players_list";
            return;
         }
         var players:Array = value;
         players.sortOn("team");
         this.playersBar.height = 25 * players.length + 2;
         this.playersBar.dataProvider = new DataProvider(players);
         this.playersBar.visible = players.length > 0;
         this.layout.reflow();
         this.freeViewButton.x = stage.stageWidth - this.freeViewButton.width - 10;
         this.freeViewButton.y = this.playersBar.y + 25 * players.length - 30;
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
         this.btn_play.selected = true;
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

