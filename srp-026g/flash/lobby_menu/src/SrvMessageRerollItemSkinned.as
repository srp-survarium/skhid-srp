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
   import survarium.messages.MessageRerollItem;
   
   public dynamic class SrvMessageRerollItemSkinned extends MessageRerollItem
   {
      
      public function SrvMessageRerollItemSkinned()
      {
         super();
         this.__setProp_modifiersList_SrvMessageRerollItemSkinned_contetn_0();
         this.__setProp_modifiersList2_SrvMessageRerollItemSkinned_contetn_0();
      }
      
      internal function __setProp_modifiersList_SrvMessageRerollItemSkinned_contetn_0() : *
      {
         try
         {
            modifiersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         modifiersList.autoSize = "none";
         modifiersList.buttonWidth = 0;
         modifiersList.direction = "vertical";
         modifiersList.enabled = true;
         modifiersList.enableInitCallback = false;
         modifiersList.focusable = true;
         modifiersList.itemRendererName = "SrvModifierLabeledRendererSlectable";
         modifiersList.spacing = 0;
         modifiersList.visible = true;
         try
         {
            modifiersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_modifiersList2_SrvMessageRerollItemSkinned_contetn_0() : *
      {
         try
         {
            modifiersList2["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         modifiersList2.autoSize = "none";
         modifiersList2.buttonWidth = 0;
         modifiersList2.direction = "vertical";
         modifiersList2.enabled = true;
         modifiersList2.enableInitCallback = false;
         modifiersList2.focusable = true;
         modifiersList2.itemRendererName = "SrvModifierLabeledRendererSlectable";
         modifiersList2.spacing = 0;
         modifiersList2.visible = true;
         try
         {
            modifiersList2["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

