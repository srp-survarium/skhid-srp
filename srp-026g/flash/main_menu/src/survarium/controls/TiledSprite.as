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
      public function set tooltipRendererName(value:String) : void
      {
         if(value == "")
         {
            return;
         }
         var tileClass:Class = getDefinitionByName(value) as Class;
         if(tileClass != null)
         {
            this.setTile(new tileClass());
         }
         else
         {
            trace("Error: " + this + ", The class " + value + " cannot be found in your library. Please ensure it is there.");
         }
      }
      
      override public function set width(value:Number) : void
      {
         super.width = value;
         this.redraw();
      }
      
      override public function set height(value:Number) : void
      {
         super.height = value;
         this.redraw();
      }
      
      public function setTile(tile:BitmapData) : void
      {
         _tile = tile;
         this.redraw();
      }
      
      private function redraw() : void
      {
         var g:Graphics;
         var matrix:Matrix;
         if(this._tile)
         {
            g = this.graphics;
            g.clear();
            matrix = new Matrix();
            matrix.scale(100 / width,100 / height);
            trace("resize::",width,height);
            g.beginBitmapFill(this._tile,matrix,true,true);
            g.drawRect(0,0,100,100);
            g.endFill();
         }
      }
   }
}

