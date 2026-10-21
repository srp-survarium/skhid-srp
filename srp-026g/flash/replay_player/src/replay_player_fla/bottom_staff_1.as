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
   
   public dynamic class bottom_staff_1 extends MovieClip
   {
      
      public var btn_next_frame:SvButton7;
      
      public var speedStepper:OptionStepperSrvBig;
      
      public var btn_next_label:SvButton9;
      
      public var btn_start:SvButton5;
      
      public var btn_play:SvButton4;
      
      public var btn_previous_frame:SvButton7;
      
      public var btn_camera:SvButtonCamera;
      
      public var btn_end:SvButton5;
      
      public var btn_previous_label:SvButton9;
      
      public var slider:SliderSrvF;
      
      public var __setPropDict:Dictionary = new Dictionary(true);
      
      public function bottom_staff_1()
      {
         super();
         addFrameScript(0,this.frame1,1,this.frame2);
      }
      
      internal function __setProp_speedStepper_bottom_staff_() : *
      {
         if(this.__setPropDict[this.speedStepper] == undefined || int(this.__setPropDict[this.speedStepper]) != 1)
         {
            this.__setPropDict[this.speedStepper] = 1;
            try
            {
               this.speedStepper["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.speedStepper.enabled = true;
            this.speedStepper.enableInitCallback = false;
            this.speedStepper.focusable = true;
            this.speedStepper.maximum = 10;
            this.speedStepper.minimum = 0;
            this.speedStepper.stepSize = 1;
            this.speedStepper.value = 0;
            this.speedStepper.visible = true;
            try
            {
               this.speedStepper["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_speedStepper_bottom_staff__1() : *
      {
         if(this.__setPropDict[this.speedStepper] == undefined || int(this.__setPropDict[this.speedStepper]) != 2)
         {
            this.__setPropDict[this.speedStepper] = 2;
            try
            {
               this.speedStepper["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.speedStepper.enabled = true;
            this.speedStepper.enableInitCallback = false;
            this.speedStepper.focusable = true;
            this.speedStepper.maximum = 10;
            this.speedStepper.minimum = 0;
            this.speedStepper.stepSize = 1;
            this.speedStepper.value = 0;
            this.speedStepper.visible = false;
            try
            {
               this.speedStepper["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_play_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_play] == undefined || int(this.__setPropDict[this.btn_play]) != 1)
         {
            this.__setPropDict[this.btn_play] = 1;
            try
            {
               this.btn_play["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_play.autoRepeat = false;
            this.btn_play.autoSize = "none";
            this.btn_play.data = "";
            this.btn_play.enabled = true;
            this.btn_play.enableInitCallback = false;
            this.btn_play.focusable = true;
            this.btn_play.label = "";
            this.btn_play.selected = false;
            this.btn_play.toggle = false;
            this.btn_play.visible = true;
            try
            {
               this.btn_play["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_play_bottom_staff__2() : *
      {
         if(this.__setPropDict[this.btn_play] == undefined || int(this.__setPropDict[this.btn_play]) != 2)
         {
            this.__setPropDict[this.btn_play] = 2;
            try
            {
               this.btn_play["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_play.autoRepeat = false;
            this.btn_play.autoSize = "none";
            this.btn_play.data = "";
            this.btn_play.enabled = true;
            this.btn_play.enableInitCallback = false;
            this.btn_play.focusable = true;
            this.btn_play.label = "";
            this.btn_play.selected = false;
            this.btn_play.toggle = false;
            this.btn_play.visible = false;
            try
            {
               this.btn_play["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_start_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_start] == undefined || int(this.__setPropDict[this.btn_start]) != 1)
         {
            this.__setPropDict[this.btn_start] = 1;
            try
            {
               this.btn_start["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_start.autoRepeat = false;
            this.btn_start.autoSize = "none";
            this.btn_start.data = "";
            this.btn_start.enabled = true;
            this.btn_start.enableInitCallback = false;
            this.btn_start.focusable = true;
            this.btn_start.label = "";
            this.btn_start.selected = false;
            this.btn_start.toggle = false;
            this.btn_start.visible = true;
            try
            {
               this.btn_start["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_start_bottom_staff__3() : *
      {
         if(this.__setPropDict[this.btn_start] == undefined || int(this.__setPropDict[this.btn_start]) != 2)
         {
            this.__setPropDict[this.btn_start] = 2;
            try
            {
               this.btn_start["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_start.autoRepeat = false;
            this.btn_start.autoSize = "none";
            this.btn_start.data = "";
            this.btn_start.enabled = true;
            this.btn_start.enableInitCallback = false;
            this.btn_start.focusable = true;
            this.btn_start.label = "";
            this.btn_start.selected = false;
            this.btn_start.toggle = false;
            this.btn_start.visible = false;
            try
            {
               this.btn_start["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_end_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_end] == undefined || int(this.__setPropDict[this.btn_end]) != 1)
         {
            this.__setPropDict[this.btn_end] = 1;
            try
            {
               this.btn_end["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_end.autoRepeat = false;
            this.btn_end.autoSize = "none";
            this.btn_end.data = "";
            this.btn_end.enabled = true;
            this.btn_end.enableInitCallback = false;
            this.btn_end.focusable = true;
            this.btn_end.label = "";
            this.btn_end.selected = false;
            this.btn_end.toggle = false;
            this.btn_end.visible = true;
            try
            {
               this.btn_end["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_end_bottom_staff__4() : *
      {
         if(this.__setPropDict[this.btn_end] == undefined || int(this.__setPropDict[this.btn_end]) != 2)
         {
            this.__setPropDict[this.btn_end] = 2;
            try
            {
               this.btn_end["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_end.autoRepeat = false;
            this.btn_end.autoSize = "none";
            this.btn_end.data = "";
            this.btn_end.enabled = true;
            this.btn_end.enableInitCallback = false;
            this.btn_end.focusable = true;
            this.btn_end.label = "";
            this.btn_end.selected = false;
            this.btn_end.toggle = false;
            this.btn_end.visible = false;
            try
            {
               this.btn_end["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_previous_frame_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_previous_frame] == undefined || int(this.__setPropDict[this.btn_previous_frame]) != 1)
         {
            this.__setPropDict[this.btn_previous_frame] = 1;
            try
            {
               this.btn_previous_frame["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_previous_frame.autoRepeat = false;
            this.btn_previous_frame.autoSize = "none";
            this.btn_previous_frame.data = "";
            this.btn_previous_frame.enabled = true;
            this.btn_previous_frame.enableInitCallback = false;
            this.btn_previous_frame.focusable = true;
            this.btn_previous_frame.label = "";
            this.btn_previous_frame.selected = false;
            this.btn_previous_frame.toggle = false;
            this.btn_previous_frame.visible = true;
            try
            {
               this.btn_previous_frame["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_previous_frame_bottom_staff__5() : *
      {
         if(this.__setPropDict[this.btn_previous_frame] == undefined || int(this.__setPropDict[this.btn_previous_frame]) != 2)
         {
            this.__setPropDict[this.btn_previous_frame] = 2;
            try
            {
               this.btn_previous_frame["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_previous_frame.autoRepeat = false;
            this.btn_previous_frame.autoSize = "none";
            this.btn_previous_frame.data = "";
            this.btn_previous_frame.enabled = true;
            this.btn_previous_frame.enableInitCallback = false;
            this.btn_previous_frame.focusable = true;
            this.btn_previous_frame.label = "";
            this.btn_previous_frame.selected = false;
            this.btn_previous_frame.toggle = false;
            this.btn_previous_frame.visible = false;
            try
            {
               this.btn_previous_frame["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_next_frame_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_next_frame] == undefined || int(this.__setPropDict[this.btn_next_frame]) != 1)
         {
            this.__setPropDict[this.btn_next_frame] = 1;
            try
            {
               this.btn_next_frame["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_next_frame.autoRepeat = false;
            this.btn_next_frame.autoSize = "none";
            this.btn_next_frame.data = "";
            this.btn_next_frame.enabled = true;
            this.btn_next_frame.enableInitCallback = false;
            this.btn_next_frame.focusable = true;
            this.btn_next_frame.label = "";
            this.btn_next_frame.selected = false;
            this.btn_next_frame.toggle = false;
            this.btn_next_frame.visible = true;
            try
            {
               this.btn_next_frame["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_next_frame_bottom_staff__6() : *
      {
         if(this.__setPropDict[this.btn_next_frame] == undefined || int(this.__setPropDict[this.btn_next_frame]) != 2)
         {
            this.__setPropDict[this.btn_next_frame] = 2;
            try
            {
               this.btn_next_frame["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_next_frame.autoRepeat = false;
            this.btn_next_frame.autoSize = "none";
            this.btn_next_frame.data = "";
            this.btn_next_frame.enabled = true;
            this.btn_next_frame.enableInitCallback = false;
            this.btn_next_frame.focusable = true;
            this.btn_next_frame.label = "";
            this.btn_next_frame.selected = false;
            this.btn_next_frame.toggle = false;
            this.btn_next_frame.visible = false;
            try
            {
               this.btn_next_frame["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_previous_label_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_previous_label] == undefined || int(this.__setPropDict[this.btn_previous_label]) != 1)
         {
            this.__setPropDict[this.btn_previous_label] = 1;
            try
            {
               this.btn_previous_label["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_previous_label.autoRepeat = false;
            this.btn_previous_label.autoSize = "none";
            this.btn_previous_label.data = "";
            this.btn_previous_label.enabled = true;
            this.btn_previous_label.enableInitCallback = false;
            this.btn_previous_label.focusable = true;
            this.btn_previous_label.label = "";
            this.btn_previous_label.selected = false;
            this.btn_previous_label.toggle = false;
            this.btn_previous_label.visible = true;
            try
            {
               this.btn_previous_label["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_previous_label_bottom_staff__7() : *
      {
         if(this.__setPropDict[this.btn_previous_label] == undefined || int(this.__setPropDict[this.btn_previous_label]) != 2)
         {
            this.__setPropDict[this.btn_previous_label] = 2;
            try
            {
               this.btn_previous_label["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_previous_label.autoRepeat = false;
            this.btn_previous_label.autoSize = "none";
            this.btn_previous_label.data = "";
            this.btn_previous_label.enabled = true;
            this.btn_previous_label.enableInitCallback = false;
            this.btn_previous_label.focusable = true;
            this.btn_previous_label.label = "";
            this.btn_previous_label.selected = false;
            this.btn_previous_label.toggle = false;
            this.btn_previous_label.visible = false;
            try
            {
               this.btn_previous_label["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_next_label_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_next_label] == undefined || int(this.__setPropDict[this.btn_next_label]) != 1)
         {
            this.__setPropDict[this.btn_next_label] = 1;
            try
            {
               this.btn_next_label["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_next_label.autoRepeat = false;
            this.btn_next_label.autoSize = "none";
            this.btn_next_label.data = "";
            this.btn_next_label.enabled = true;
            this.btn_next_label.enableInitCallback = false;
            this.btn_next_label.focusable = true;
            this.btn_next_label.label = "";
            this.btn_next_label.selected = false;
            this.btn_next_label.toggle = false;
            this.btn_next_label.visible = true;
            try
            {
               this.btn_next_label["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_next_label_bottom_staff__8() : *
      {
         if(this.__setPropDict[this.btn_next_label] == undefined || int(this.__setPropDict[this.btn_next_label]) != 2)
         {
            this.__setPropDict[this.btn_next_label] = 2;
            try
            {
               this.btn_next_label["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_next_label.autoRepeat = false;
            this.btn_next_label.autoSize = "none";
            this.btn_next_label.data = "";
            this.btn_next_label.enabled = true;
            this.btn_next_label.enableInitCallback = false;
            this.btn_next_label.focusable = true;
            this.btn_next_label.label = "";
            this.btn_next_label.selected = false;
            this.btn_next_label.toggle = false;
            this.btn_next_label.visible = false;
            try
            {
               this.btn_next_label["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_camera_bottom_staff_() : *
      {
         if(this.__setPropDict[this.btn_camera] == undefined || int(this.__setPropDict[this.btn_camera]) != 1)
         {
            this.__setPropDict[this.btn_camera] = 1;
            try
            {
               this.btn_camera["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_camera.autoRepeat = false;
            this.btn_camera.autoSize = "none";
            this.btn_camera.data = "";
            this.btn_camera.enabled = true;
            this.btn_camera.enableInitCallback = false;
            this.btn_camera.focusable = true;
            this.btn_camera.label = "";
            this.btn_camera.selected = false;
            this.btn_camera.toggle = false;
            this.btn_camera.visible = true;
            try
            {
               this.btn_camera["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_btn_camera_bottom_staff__9() : *
      {
         if(this.__setPropDict[this.btn_camera] == undefined || int(this.__setPropDict[this.btn_camera]) != 2)
         {
            this.__setPropDict[this.btn_camera] = 2;
            try
            {
               this.btn_camera["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.btn_camera.autoRepeat = false;
            this.btn_camera.autoSize = "none";
            this.btn_camera.data = "";
            this.btn_camera.enabled = true;
            this.btn_camera.enableInitCallback = false;
            this.btn_camera.focusable = true;
            this.btn_camera.label = "";
            this.btn_camera.selected = false;
            this.btn_camera.toggle = false;
            this.btn_camera.visible = false;
            try
            {
               this.btn_camera["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function frame1() : *
      {
         this.__setProp_btn_camera_bottom_staff_();
         this.__setProp_btn_next_label_bottom_staff_();
         this.__setProp_btn_previous_label_bottom_staff_();
         this.__setProp_btn_next_frame_bottom_staff_();
         this.__setProp_btn_previous_frame_bottom_staff_();
         this.__setProp_btn_end_bottom_staff_();
         this.__setProp_btn_start_bottom_staff_();
         this.__setProp_btn_play_bottom_staff_();
         this.__setProp_speedStepper_bottom_staff_();
         stop();
      }
      
      internal function frame2() : *
      {
         this.__setProp_btn_camera_bottom_staff__9();
         this.__setProp_btn_next_label_bottom_staff__8();
         this.__setProp_btn_previous_label_bottom_staff__7();
         this.__setProp_btn_next_frame_bottom_staff__6();
         this.__setProp_btn_previous_frame_bottom_staff__5();
         this.__setProp_btn_end_bottom_staff__4();
         this.__setProp_btn_start_bottom_staff__3();
         this.__setProp_btn_play_bottom_staff__2();
         this.__setProp_speedStepper_bottom_staff__1();
      }
   }
}

