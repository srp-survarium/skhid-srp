package survarium.core
{
   import flash.display.Bitmap;
   import flash.display.MovieClip;
   import flash.events.Event;
   
   public class Icon extends MovieClip
   {
      
      protected var _bmp:Bitmap;
      
      public function Icon()
      {
         super();
      }
      
      private function onAdded(e:Event) : void
      {
         trace("Icon::",e.type,name,e.target,e.currentTarget,parent,this._bmp,numChildren);
      }
      
      public function get bitmap() : Bitmap
      {
         return this._bmp;
      }
      
      public function applyImage(icon:Icon) : void
      {
         if(icon.bitmap)
         {
            this.loadImage(new Bitmap(icon.bitmap.bitmapData));
         }
      }
      
      public function reload() : void
      {
         if(this._bmp)
         {
            addChild(this._bmp);
         }
      }
      
      public function clear() : void
      {
         if(!this._bmp)
         {
            return;
         }
         this._bmp.bitmapData = null;
         removeChild(this._bmp);
         _bmp = null;
      }
      
      public function loadImage(bmp:Bitmap) : void
      {
         if(this._bmp != null)
         {
            removeChild(this._bmp);
         }
         _bmp = bmp;
         addChild(this._bmp);
      }
      
      public function clone() : Icon
      {
         var _clone:Icon = new Icon();
         if(this._bmp)
         {
            _clone.loadImage(new Bitmap(this._bmp.bitmapData));
         }
         return _clone;
      }
      
      override public function toString() : String
      {
         return "[Icon " + name + "]";
      }
   }
}

