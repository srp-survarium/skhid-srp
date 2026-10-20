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
   import survarium.messages.MessageTutorialComplete;
   
   public dynamic class MessageTutorialComplete extends survarium.messages.MessageTutorialComplete
   {
      
      public function MessageTutorialComplete()
      {
         super();
         this.__setProp_btnAccept_MessageTutorialComplete_();
         this.__setProp_btnCancel_MessageTutorialComplete_();
      }
      
      internal function __setProp_btnAccept_MessageTutorialComplete_() : *
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
         btnAccept.label = "label";
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
      
      internal function __setProp_btnCancel_MessageTutorialComplete_() : *
      {
         try
         {
            btnCancel["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnCancel.autoRepeat = false;
         btnCancel.autoSize = "none";
         btnCancel.data = "";
         btnCancel.enabled = true;
         btnCancel.enableInitCallback = false;
         btnCancel.focusable = true;
         btnCancel.label = "label";
         btnCancel.selected = false;
         btnCancel.tooltipRendererName = "";
         btnCancel.toggle = false;
         btnCancel.visible = false;
         try
         {
            btnCancel["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

