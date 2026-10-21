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
   import survarium.chat.GameChat;
   
   public dynamic class SrvGameChat extends GameChat
   {
      
      public function SrvGameChat()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_text_inpput_SrvGameChat_Layer1_0();
      }
      
      internal function __setProp_text_inpput_SrvGameChat_Layer1_0() : *
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
      
      internal function frame1() : *
      {
         stop();
         text_inpput.defaultText = "";
         textFieldName.autoSize = TextFieldAutoSize.LEFT;
         textFieldName.visible = false;
      }
   }
}

