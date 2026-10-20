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
   import survarium.chat.LobbyChatTab;
   
   public dynamic class SrvChatTab extends LobbyChatTab
   {
      
      public function SrvChatTab()
      {
         super();
         this.__setProp_text_inpput_SrvChatTab_Layer1_0();
         this.__setProp_text_chat_SrvChatTab_Layer4_0();
         this.__setProp_labelTextField_SrvChatTab_Layer2_0();
      }
      
      internal function __setProp_text_inpput_SrvChatTab_Layer1_0() : *
      {
         try
         {
            text_inpput["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         text_inpput.actAsButton = true;
         text_inpput.defaultText = "enter message";
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
      
      internal function __setProp_text_chat_SrvChatTab_Layer4_0() : *
      {
         try
         {
            text_chat["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         text_chat.actAsButton = false;
         text_chat.defaultText = "";
         text_chat.displayAsPassword = false;
         text_chat.editable = false;
         text_chat.enabled = true;
         text_chat.enableInitCallback = false;
         text_chat.focusable = true;
         text_chat.maxChars = 0;
         text_chat.minThumbSize = 15;
         text_chat.scrollBar = "SrvScrollBarSkinned";
         text_chat.text = "";
         text_chat.thumbOffset = {
            "top":0,
            "bottom":0
         };
         text_chat.visible = true;
         try
         {
            text_chat["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_labelTextField_SrvChatTab_Layer2_0() : *
      {
         try
         {
            labelTextField["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         labelTextField.text = "";
         try
         {
            labelTextField["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

