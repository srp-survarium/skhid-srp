package survarium.controls
{
   import flash.display.BitmapData;
   import flash.display.Graphics;
   import flash.display.Sprite;
   import flash.geom.Matrix;
   import flash.utils.getDefinitionByName;
   
   public class TiledSprite extends Sprite
   {
      
      private var _tile:BitmapData;
      
      public function TiledSprite()
      {
         super();
      }
      
      [Inspectable(name="tileRenderer",defaultValue="")]
      public function set tooltipRendererName(param1:String) : void
      {
         if(param1 == "")
         {
            return;
         }
         var _loc2_:Class = getDefinitionByName(param1) as Class;
         if(_loc2_ != null)
         {
            this.setTile(new _loc2_());
         }
         else
         {
            trace("Error: " + this + ", The class " + param1 + " cannot be found in your library. Please ensure it is there.");
         }
      }
      
      override public function set width(param1:Number) : void
      {
         super.width = param1;
         this.redraw();
      }
      
      override public function set height(param1:Number) : void
      {
         super.height = param1;
         this.redraw();
      }
      
      public function setTile(param1:BitmapData) : void
      {
         _tile = param1;
         this.redraw();
      }
      
      private function redraw() : void
      {
         var _loc1_:Graphics;
         var _loc2_:Matrix;
         if(this._tile)
         {
            _loc1_ = this.graphics;
            _loc1_.clear();
            _loc2_ = new Matrix();
            _loc2_.scale(100 / width,100 / height);
            trace("resize::",width,height);
            _loc1_.beginBitmapFill(this._tile,_loc2_,true,true);
            _loc1_.drawRect(0,0,100,100);
            _loc1_.endFill();
         }
      }
   }
}

