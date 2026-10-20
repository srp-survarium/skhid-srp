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
   import survarium.controls.PlayerSearchComponent;
   
   public dynamic class SrvPlayerSearchComponentClanOld extends PlayerSearchComponent
   {
      
      public function SrvPlayerSearchComponentClanOld()
      {
         super();
         this.__setProp_textInput_SrvPlayerSearchComponentClanOld_enterField_0();
         this.__setProp_playersList_SrvPlayerSearchComponentClanOld_test_0();
      }
      
      internal function __setProp_textInput_SrvPlayerSearchComponentClanOld_enterField_0() : *
      {
         try
         {
            textInput["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         textInput.actAsButton = true;
         textInput.defaultText = "";
         textInput.displayAsPassword = false;
         textInput.editable = true;
         textInput.enabled = true;
         textInput.enableInitCallback = false;
         textInput.focusable = true;
         textInput.maxChars = 120;
         textInput.text = "";
         textInput.visible = true;
         try
         {
            textInput["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_playersList_SrvPlayerSearchComponentClanOld_test_0() : *
      {
         try
         {
            playersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         playersList.draggable = false;
         playersList.enabled = true;
         playersList.enableInitCallback = false;
         playersList.focusable = true;
         playersList.itemRendererName = "SrvFriendListRenderSlotSearch";
         playersList.itemRendererInstanceName = "";
         playersList.margin = 2;
         playersList.inspectablePadding = {
            "top":4,
            "right":4,
            "bottom":4,
            "left":4
         };
         playersList.rowHeight = 35;
         playersList.scrollBar = "SrvScrollBar";
         playersList.scrollYPadding = 0;
         playersList.visible = true;
         playersList.wrapping = "normal";
         try
         {
            playersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

