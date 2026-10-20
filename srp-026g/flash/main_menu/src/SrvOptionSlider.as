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
   import survarium.menu.OptionSlider;
   
   public dynamic class SrvOptionSlider extends OptionSlider
   {
      
      public function SrvOptionSlider()
      {
         super();
         this.__setProp_slider_option_slider_();
      }
      
      internal function __setProp_slider_option_slider_() : *
      {
         try
         {
            slider["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         slider.enabled = true;
         slider.enableInitCallback = false;
         slider.focusable = true;
         slider.liveDragging = true;
         slider.maximum = 4;
         slider.minimum = 1;
         slider.offsetLeft = 10;
         slider.offsetRight = 20;
         slider.snapInterval = 1;
         slider.snapping = true;
         slider.value = 0;
         slider.visible = true;
         try
         {
            slider["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

