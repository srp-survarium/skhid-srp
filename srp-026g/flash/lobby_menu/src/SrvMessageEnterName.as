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
   import survarium.messages.MessageRename;
   
   public dynamic class SrvMessageEnterName extends MessageRename
   {
      
      public function SrvMessageEnterName()
      {
         super();
         this.__setProp_textInput_SrvMessageEnterName_();
      }
      
      internal function __setProp_textInput_SrvMessageEnterName_() : *
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
   }
}

