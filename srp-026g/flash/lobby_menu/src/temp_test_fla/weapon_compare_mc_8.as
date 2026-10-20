package temp_test_fla
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
   
   public dynamic class weapon_compare_mc_8 extends MovieClip
   {
      
      public var background:MovieClip;
      
      public var indicator:HintDiffIndicator;
      
      public function weapon_compare_mc_8()
      {
         super();
         this.__setProp_indicator_weapon_compare_mc_();
      }
      
      internal function __setProp_indicator_weapon_compare_mc_() : *
      {
         try
         {
            this.indicator["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.indicator.enabled = true;
         this.indicator.enableInitCallback = false;
         this.indicator.maximum = 100;
         this.indicator.minimum = 0;
         this.indicator.value = 80;
         this.indicator.visible = true;
         try
         {
            this.indicator["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

