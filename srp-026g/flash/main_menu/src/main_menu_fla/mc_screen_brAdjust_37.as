package main_menu_fla
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
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.FocusManager;
   import survarium.menu.OptionSlider;
   
   public dynamic class mc_screen_brAdjust_37 extends MovieClip
   {
      
      public var background:MovieClip;
      
      public var btn_set:InventoryToggleButton;
      
      public var slider_gamma:OptionSlider;
      
      public function mc_screen_brAdjust_37()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_btn_set_mc_screen_brAdjust_Layer1_0();
      }
      
      public function btn_set_action(param1:ButtonEvent = null) : void
      {
         visible = false;
         FocusManager.setModalClip(stage as Sprite);
         stage.focus = stage;
         parent.removeChild(this);
      }
      
      public function setSize(param1:Number, param2:Number) : *
      {
         this.background.width = param1;
         this.background.height = param2;
         this.slider_gamma.y = param2 - 43;
         this.slider_gamma.x = int(param1 / 2 - this.slider_gamma.width);
         this.btn_set.y = param2 - 43;
         this.btn_set.x = param1 - 200;
      }
      
      internal function __setProp_btn_set_mc_screen_brAdjust_Layer1_0() : *
      {
         try
         {
            this.btn_set["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btn_set.autoRepeat = false;
         this.btn_set.autoSize = "none";
         this.btn_set.data = "";
         this.btn_set.enabled = true;
         this.btn_set.enableInitCallback = false;
         this.btn_set.focusable = true;
         this.btn_set.label = "Отмена";
         this.btn_set.selected = false;
         this.btn_set.toggle = false;
         this.btn_set.visible = true;
         try
         {
            this.btn_set["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
         this.btn_set.addEventListener(ButtonEvent.CLICK,this.btn_set_action,false,0,true);
      }
   }
}

