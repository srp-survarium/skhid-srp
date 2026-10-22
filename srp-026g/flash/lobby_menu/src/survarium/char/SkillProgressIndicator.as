package survarium.char
{
   import flash.display.MovieClip;
   import flash.geom.Point;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.StatusIndicator;
   
   public class SkillProgressIndicator extends StatusIndicator
   {
      
      public var radius:uint = 41;
      
      protected var _progressRenderer:Class;
      
      protected var _renderers:Array = [];
      
      public function SkillProgressIndicator()
      {
         super();
      }
      
      public function registerRenderer(rendererClass:Class) : void
      {
         _progressRenderer = rendererClass;
         invalidateData();
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateRenderers();
            this.updatePosition();
         }
      }
      
      override protected function updatePosition() : void
      {
         var renderer:MovieClip;
         var count:int = this._renderers.length;
         var i:uint = 0;
         while(i < count)
         {
            renderer = this._renderers[i];
            renderer.gotoAndStop(i < _value ? 1 : 2);
            i++;
         }
      }
      
      protected function updateRenderers() : void
      {
         var count:int = _maximum;
         var angleStep:uint = 20;
         var renderer:MovieClip;
         trace("updateRenderers",_maximum,this._progressRenderer);
         if(!this._progressRenderer)
         {
            return;
         }
         var angle:int = -90 - count / 2 * angleStep + angleStep / 2;
         var i:uint = count;
         while(i < this._renderers.length)
         {
            renderer = this._renderers[i];
            renderer.gotoAndStop(1);
            removeChild(renderer);
            i++;
         }
         var j:uint = 0;
         var radians:Number;
         var direction:Point;
         var posX:Number;
         var posY:Number;
         while(j < count)
         {
            if(!this._renderers[j])
            {
               renderer = new this._progressRenderer();
               renderer.gotoAndStop(2);
               this._renderers[j] = renderer;
               trace("SPI:creteRednderer",renderer);
            }
            else
            {
               renderer = this._renderers[j];
               trace("SPI:updateRenderer",renderer);
            }
            addChild(renderer);
            radians = angle * (Math.PI / 180);
            direction = new Point(Math.cos(radians),Math.sin(radians));
            posX = direction.x * this.radius;
            posY = -this.radius + -direction.y * this.radius;
            renderer.x = posX;
            renderer.y = posY;
            angle += angleStep;
            j++;
         }
      }
   }
}

