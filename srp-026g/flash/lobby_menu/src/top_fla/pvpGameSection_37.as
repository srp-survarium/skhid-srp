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
   
   public dynamic class pvpGameSection_37 extends MovieClip
   {
      
      public var checkBoxTeam:CheckBoxSkinnedIconDev;
      
      public var sectionBack:MovieClip;
      
      public var checkBoxCharging:CheckBoxSkinnedIcon;
      
      public var checkBoxPVP:RadioSharedTop;
      
      public function pvpGameSection_37()
      {
         super();
         this.__setProp_checkBoxCharging_pvpGameSection_();
         this.__setProp_checkBoxTeam_pvpGameSection_();
         this.__setProp_checkBoxPVP_pvpGameSection_();
      }
      
      internal function __setProp_checkBoxCharging_pvpGameSection_() : *
      {
         try
         {
            this.checkBoxCharging["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkBoxCharging.autoSize = "none";
         this.checkBoxCharging.data = "";
         this.checkBoxCharging.enabled = false;
         this.checkBoxCharging.enableInitCallback = false;
         this.checkBoxCharging.focusable = true;
         this.checkBoxCharging.label = "";
         this.checkBoxCharging.selected = false;
         this.checkBoxCharging.visible = true;
         try
         {
            this.checkBoxCharging["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_checkBoxTeam_pvpGameSection_() : *
      {
         try
         {
            this.checkBoxTeam["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkBoxTeam.autoSize = "none";
         this.checkBoxTeam.data = "";
         this.checkBoxTeam.enabled = false;
         this.checkBoxTeam.enableInitCallback = false;
         this.checkBoxTeam.focusable = true;
         this.checkBoxTeam.label = "";
         this.checkBoxTeam.selected = false;
         this.checkBoxTeam.visible = true;
         try
         {
            this.checkBoxTeam["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_checkBoxPVP_pvpGameSection_() : *
      {
         try
         {
            this.checkBoxPVP["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkBoxPVP.autoSize = "none";
         this.checkBoxPVP.data = "";
         this.checkBoxPVP.enabled = false;
         this.checkBoxPVP.enableInitCallback = false;
         this.checkBoxPVP.focusable = true;
         this.checkBoxPVP.label = "";
         this.checkBoxPVP.selected = true;
         this.checkBoxPVP.visible = true;
         try
         {
            this.checkBoxPVP["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

