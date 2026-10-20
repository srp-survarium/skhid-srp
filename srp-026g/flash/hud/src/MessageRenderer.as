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
   
   public dynamic class MessageRenderer extends MovieClip
   {
      
      public var textContainer:MovieClip;
      
      public function MessageRenderer()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function clear(e:Object = null) : void
      {
         parent.removeChild(this);
      }
      
      internal function frame1() : *
      {
         this.textContainer.textField.defaultTextFormat = this.textContainer.textField.getTextFormat();
         this.textContainer.textField.autoSize = TextFieldAutoSize.LEFT;
      }
   }
}

