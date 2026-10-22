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
   import survarium.friends.FriendsMenu;
   
   public dynamic class SrvFriendsMenuReskin extends FriendsMenu
   {
      
      public function SrvFriendsMenuReskin()
      {
         super();
         this.__setProp_playersList_SrvFriendsMenuReskin_Layer1_0();
         this.__setProp_text_inpput_SrvFriendsMenuReskin_();
      }
      
      internal function __setProp_playersList_SrvFriendsMenuReskin_Layer1_0() : *
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
         playersList.itemRendererName = "SrvFriendListRender";
         playersList.itemRendererInstanceName = "";
         playersList.margin = 2;
         playersList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         playersList.rowHeight = 35;
         playersList.scrollBar = "";
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
      
      internal function __setProp_text_inpput_SrvFriendsMenuReskin_() : *
      {
         try
         {
            text_inpput["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         text_inpput.actAsButton = true;
         text_inpput.defaultText = "";
         text_inpput.displayAsPassword = false;
         text_inpput.editable = true;
         text_inpput.enabled = true;
         text_inpput.enableInitCallback = false;
         text_inpput.focusable = true;
         text_inpput.maxChars = 120;
         text_inpput.text = "";
         text_inpput.visible = true;
         try
         {
            text_inpput["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

