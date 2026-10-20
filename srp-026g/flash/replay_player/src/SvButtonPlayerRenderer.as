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
   import survarium.buttons.PlayerViewButton;
   
   public dynamic class SvButtonPlayerRenderer extends PlayerViewButton
   {
      
      public function SvButtonPlayerRenderer()
      {
         super();
         addFrameScript(0,this.frame1,9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40,49,this.frame50,59,this.frame60,69,this.frame70,79,this.frame80);
         this.__setProp_btnFirstView_SvButtonPlayerRenderer_();
         this.__setProp_btnThirdView_SvButtonPlayerRenderer_();
      }
      
      internal function __setProp_btnFirstView_SvButtonPlayerRenderer_() : *
      {
         try
         {
            btnFirstView["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnFirstView.autoRepeat = false;
         btnFirstView.autoSize = "none";
         btnFirstView.data = "";
         btnFirstView.enabled = true;
         btnFirstView.enableInitCallback = false;
         btnFirstView.focusable = true;
         btnFirstView.label = "1st";
         btnFirstView.selected = false;
         btnFirstView.toggle = false;
         btnFirstView.visible = true;
         try
         {
            btnFirstView["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnThirdView_SvButtonPlayerRenderer_() : *
      {
         try
         {
            btnThirdView["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btnThirdView.autoRepeat = false;
         btnThirdView.autoSize = "none";
         btnThirdView.data = "";
         btnThirdView.enabled = true;
         btnThirdView.enableInitCallback = false;
         btnThirdView.focusable = true;
         btnThirdView.label = "3rd";
         btnThirdView.selected = false;
         btnThirdView.toggle = false;
         btnThirdView.visible = true;
         try
         {
            btnThirdView["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
         textField.defaultTextFormat = textField.getTextFormat();
      }
      
      internal function frame10() : *
      {
         stop();
      }
      
      internal function frame20() : *
      {
         stop();
      }
      
      internal function frame30() : *
      {
         stop();
      }
      
      internal function frame40() : *
      {
         stop();
      }
      
      internal function frame50() : *
      {
         stop();
      }
      
      internal function frame60() : *
      {
         stop();
      }
      
      internal function frame70() : *
      {
         stop();
      }
      
      internal function frame80() : *
      {
         stop();
      }
   }
}

