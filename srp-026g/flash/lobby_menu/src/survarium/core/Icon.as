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
      
      private function onAdded(param1:Event) : void
      {
         trace("Icon::",param1.type,name,param1.target,param1.currentTarget,parent,this._bmp,numChildren);
      }
      
      public function get bitmap() : Bitmap
      {
         return this._bmp;
      }
      
      public function applyImage(param1:Icon) : void
      {
         if(param1.bitmap)
         {
            this.loadImage(new Bitmap(param1.bitmap.bitmapData));
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
      
      public function loadImage(param1:Bitmap) : void
      {
         if(this._bmp != null)
         {
            removeChild(this._bmp);
         }
         _bmp = param1;
         addChild(this._bmp);
      }
      
      public function clone() : Icon
      {
         var _loc1_:Icon = new Icon();
         if(this._bmp)
         {
            _loc1_.loadImage(new Bitmap(this._bmp.bitmapData));
         }
         return _loc1_;
      }
      
      override public function toString() : String
      {
         return "[Icon " + name + "]";
      }
   }
}

