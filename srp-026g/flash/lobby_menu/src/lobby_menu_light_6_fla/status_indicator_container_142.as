package lobby_menu_light_6_fla
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
   
   public dynamic class status_indicator_container_142 extends MovieClip
   {
      
      public var _current:MovieClip;
      
      public var _percent:TextField;
      
      public var progress_bar:PlayerStateIndicatorProgress_mm;
      
      public var _total:MovieClip;
      
      public function status_indicator_container_142()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_progress_bar_status_indicator_container_Layer1_0();
      }
      
      public function set_progress(param1:int, param2:int) : void
      {
         if(param1 > param2)
         {
            return;
         }
         this._total.visible = true;
         this._current.visible = true;
         var _loc3_:int = param1 / param2 * 100;
         this._percent.text = _loc3_.toString() + "%";
         this.progress_bar.value = _loc3_;
         this.progress_bar.progress = 0;
         this._current.tx.text = param1.toString();
         this._current.x = int(this.progress_bar.x - 3 + this.progress_bar.width * (this.progress_bar.value / 100));
         this._total.tx.text = param2.toString();
      }
      
      public function set_percent(param1:int) : void
      {
         if(param1 > 100)
         {
            return;
         }
         var _loc2_:int = param1;
         this._percent.text = _loc2_.toString() + "%";
         if(_loc2_ == 100)
         {
         }
         this.progress_bar.value = _loc2_;
         this.progress_bar.progress = 0;
      }
      
      internal function __setProp_progress_bar_status_indicator_container_Layer1_0() : *
      {
         try
         {
            this.progress_bar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.progress_bar.enabled = true;
         this.progress_bar.enableInitCallback = false;
         this.progress_bar.maximum = 100;
         this.progress_bar.minimum = 0;
         this.progress_bar.value = 0;
         this.progress_bar.visible = true;
         try
         {
            this.progress_bar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
         this._total.visible = false;
         this._current.visible = false;
         this.set_percent(0);
      }
   }
}

