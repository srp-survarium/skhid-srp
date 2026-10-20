package panel_fla
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
   
   public dynamic class WeightContainer_44 extends MovieClip
   {
      
      public var progress_indicator:WeigthProgressIndicator;
      
      public var textFieldLabel:TextField;
      
      public var textFieldValue:TextField;
      
      public function WeightContainer_44()
      {
         super();
         this.__setProp_progress_indicator_WeightContainer_();
      }
      
      internal function __setProp_progress_indicator_WeightContainer_() : *
      {
         try
         {
            this.progress_indicator["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.progress_indicator.enabled = true;
         this.progress_indicator.enableInitCallback = false;
         this.progress_indicator.maximum = 100;
         this.progress_indicator.minimum = 0;
         this.progress_indicator.value = 0;
         this.progress_indicator.visible = true;
         try
         {
            this.progress_indicator["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

