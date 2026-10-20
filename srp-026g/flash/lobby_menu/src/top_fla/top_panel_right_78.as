package top_fla
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
   
   public dynamic class top_panel_right_78 extends MovieClip
   {
      
      public var btnShowDailyMessage:btnDailyMessage;
      
      public var factionHint:hitnArea;
      
      public var factionProgressIndicator:TopFactionProgressIndicator;
      
      public var textFieldFactionName:TextField;
      
      public var hintButtonFactionProgress:hitnArea;
      
      public var textFieldFactionProgress:TextField;
      
      public var btnActiveFaction:QuestButton;
      
      public function top_panel_right_78()
      {
         super();
         this.__setProp_factionProgressIndicator_top_panel_right_();
         this.__setProp_factionHint_top_panel_right_();
      }
      
      internal function __setProp_factionProgressIndicator_top_panel_right_() : *
      {
         try
         {
            this.factionProgressIndicator["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.factionProgressIndicator.enabled = true;
         this.factionProgressIndicator.enableInitCallback = false;
         this.factionProgressIndicator.maximum = 100;
         this.factionProgressIndicator.minimum = 0;
         this.factionProgressIndicator.value = 0;
         this.factionProgressIndicator.visible = true;
         try
         {
            this.factionProgressIndicator["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_factionHint_top_panel_right_() : *
      {
         try
         {
            this.factionHint["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.factionHint.enabled = true;
         this.factionHint.enableInitCallback = false;
         this.factionHint.tooltipRendererName = "SrvProfileTipRenderer";
         this.factionHint.visible = true;
         try
         {
            this.factionHint["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

