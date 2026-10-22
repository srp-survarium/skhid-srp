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
   
   public dynamic class modifiersContainer_13 extends MovieClip
   {
      
      public var camoSlot:SrvModifierRendererSk;
      
      public var background:MovieClip;
      
      public var decalSlot:SrvModifierRendererDecalSk;
      
      public var btnCancel:SrvButtonSkinnedSk;
      
      public var textField:TextField;
      
      public var decalSlot2:SrvModifierRendererDecalSk;
      
      public var decalSlot1:SrvModifierRendererDecalSk;
      
      public function modifiersContainer_13()
      {
         super();
         this.__setProp_btnCancel_modifiersContainer_();
      }
      
      internal function __setProp_btnCancel_modifiersContainer_() : *
      {
         try
         {
            this.btnCancel["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btnCancel.autoRepeat = false;
         this.btnCancel.autoSize = "none";
         this.btnCancel.data = "";
         this.btnCancel.enabled = true;
         this.btnCancel.enableInitCallback = false;
         this.btnCancel.focusable = true;
         this.btnCancel.label = "label";
         this.btnCancel.selected = false;
         this.btnCancel.tooltipRendererName = "";
         this.btnCancel.toggle = false;
         this.btnCancel.visible = true;
         try
         {
            this.btnCancel["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

