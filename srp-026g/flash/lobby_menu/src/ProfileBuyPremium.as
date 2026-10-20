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
   import survarium.messages.MessagePremiumProfile;
   
   public dynamic class ProfileBuyPremium extends MessagePremiumProfile
   {
      
      public function ProfileBuyPremium()
      {
         super();
         this.__setProp_dropDownlist_profileBuyPremium_();
         this.__setProp_btnAccept_profileBuyPremium_();
      }
      
      internal function __setProp_dropDownlist_profileBuyPremium_() : *
      {
         try
         {
            dropDownlist["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         dropDownlist.autoSize = "none";
         dropDownlist.dropdown = "OptionScrollingList";
         dropDownlist.enabled = true;
         dropDownlist.enableInitCallback = false;
         dropDownlist.focusable = true;
         dropDownlist.itemRenderer = "OptionsListRenderer";
         dropDownlist.menuDirection = "down";
         dropDownlist.menuMargin = 0;
         dropDownlist.inspectableMenuOffset = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         dropDownlist.inspectableMenuPadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         dropDownlist.menuRowCount = 10;
         dropDownlist.menuRowsFixed = false;
         dropDownlist.menuWidth = 200;
         dropDownlist.menuWrapping = "normal";
         dropDownlist.scrollBar = "";
         dropDownlist.inspectableThumbOffset = {
            "top":0,
            "bottom":0
         };
         dropDownlist.tooltipRendererName = "";
         dropDownlist.visible = true;
         try
         {
            dropDownlist["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnAccept_profileBuyPremium_() : *
      {
         try
         {
            btnAccept["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnAccept.autoRepeat = false;
         btnAccept.autoSize = "none";
         btnAccept.data = "";
         btnAccept.enabled = true;
         btnAccept.enableInitCallback = false;
         btnAccept.focusable = true;
         btnAccept.label = "red_label";
         btnAccept.selected = false;
         btnAccept.tooltipRendererName = "";
         btnAccept.toggle = false;
         btnAccept.visible = true;
         try
         {
            btnAccept["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

