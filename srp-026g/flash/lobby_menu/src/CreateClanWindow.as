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
   import survarium.clan.CreateClanWindow;
   
   public dynamic class CreateClanWindow extends survarium.clan.CreateClanWindow
   {
      
      public function CreateClanWindow()
      {
         super();
         this.__setProp_textInputClanName_CreateClanWindow_();
         this.__setProp_textInputClanAbbreviation_CreateClanWindow_();
         this.__setProp_clanInvitesList_CreateClanWindow_();
      }
      
      internal function __setProp_textInputClanName_CreateClanWindow_() : *
      {
         try
         {
            textInputClanName["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         textInputClanName.actAsButton = true;
         textInputClanName.defaultText = "";
         textInputClanName.displayAsPassword = false;
         textInputClanName.editable = true;
         textInputClanName.enabled = true;
         textInputClanName.enableInitCallback = false;
         textInputClanName.focusable = true;
         textInputClanName.maxChars = 30;
         textInputClanName.text = "";
         textInputClanName.visible = true;
         try
         {
            textInputClanName["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_textInputClanAbbreviation_CreateClanWindow_() : *
      {
         try
         {
            textInputClanAbbreviation["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         textInputClanAbbreviation.actAsButton = true;
         textInputClanAbbreviation.defaultText = "";
         textInputClanAbbreviation.displayAsPassword = false;
         textInputClanAbbreviation.editable = true;
         textInputClanAbbreviation.enabled = true;
         textInputClanAbbreviation.enableInitCallback = false;
         textInputClanAbbreviation.focusable = true;
         textInputClanAbbreviation.maxChars = 6;
         textInputClanAbbreviation.text = "";
         textInputClanAbbreviation.visible = true;
         try
         {
            textInputClanAbbreviation["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_clanInvitesList_CreateClanWindow_() : *
      {
         try
         {
            clanInvitesList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         clanInvitesList.enabled = true;
         clanInvitesList.enableInitCallback = false;
         clanInvitesList.focusable = true;
         clanInvitesList.itemRendererName = "SrvClanInviteRenderer";
         clanInvitesList.itemRendererInstanceName = "";
         clanInvitesList.margin = 0;
         clanInvitesList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         clanInvitesList.rowHeight = 34;
         clanInvitesList.scrollBar = "SrvScrollBarSkinned";
         clanInvitesList.visible = true;
         clanInvitesList.wrapping = "normal";
         try
         {
            clanInvitesList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

