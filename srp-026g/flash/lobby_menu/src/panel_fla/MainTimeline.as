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
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var slotList:SrvListItemRenderer;
      
      public var slotWeapon:SrvInvRenderer;
      
      public var checkbox_autobuy:CheckBoxSharedPD;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_checkbox_autobuy_Scene1_paperdoll_ful_0();
      }
      
      internal function __setProp_checkbox_autobuy_Scene1_paperdoll_ful_0() : *
      {
         try
         {
            this.checkbox_autobuy["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkbox_autobuy.autoSize = "none";
         this.checkbox_autobuy.data = "";
         this.checkbox_autobuy.enabled = false;
         this.checkbox_autobuy.enableInitCallback = false;
         this.checkbox_autobuy.focusable = true;
         this.checkbox_autobuy.label = "";
         this.checkbox_autobuy.selected = false;
         this.checkbox_autobuy.visible = true;
         try
         {
            this.checkbox_autobuy["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
      }
   }
}

