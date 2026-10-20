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
   
   public dynamic class SrvOxygenContainer extends MovieClip
   {
      
      public var progress_indicator:MovieClip;
      
      public var textField_progreess:TextField;
      
      public function SrvOxygenContainer()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set_oxygen(percent:uint, time:uint) : void
      {
         trace("set_oxygen::",percent,time);
         this.progress_indicator.set_progress(percent / 100);
         var tm:Number = time / 1000;
         this.textField_progreess.text = tm.toFixed(1).toString();
         this.textField_progreess.x = this.progress_indicator.x + this.progress_indicator.progress_thumb.width - this.textField_progreess.textWidth / 2;
      }
      
      internal function frame1() : *
      {
      }
   }
}

