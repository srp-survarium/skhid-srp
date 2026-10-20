package
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
   import survarium.debug.FontDebugControl;
   
   public dynamic class SrvFontDebugControl extends FontDebugControl
   {
      
      public function SrvFontDebugControl()
      {
         super();
         this.__setProp_stepper_SrvFontDebugControl_();
         this.__setProp_reset_SrvFontDebugControl_();
      }
      
      internal function __setProp_stepper_SrvFontDebugControl_() : *
      {
         try
         {
            stepper["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         stepper.enabled = true;
         stepper.enableInitCallback = false;
         stepper.focusable = true;
         stepper.maximum = 10;
         stepper.minimum = -10;
         stepper.stepSize = 0.5;
         stepper.value = 0;
         stepper.visible = true;
         try
         {
            stepper["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_reset_SrvFontDebugControl_() : *
      {
         try
         {
            reset["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         reset.autoRepeat = false;
         reset.autoSize = "none";
         reset.data = "";
         reset.enabled = true;
         reset.enableInitCallback = false;
         reset.focusable = true;
         reset.label = "R";
         reset.selected = false;
         reset.tooltipRendererName = "";
         reset.toggle = false;
         reset.visible = true;
         try
         {
            reset["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

