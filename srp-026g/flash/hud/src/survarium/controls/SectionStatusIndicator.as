package survarium.controls
{
   import flash.display.MovieClip;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.StatusIndicator;
   
   public class SectionStatusIndicator extends StatusIndicator
   {
      
      protected var _progressRenderer:Class;
      
      protected var _rendererStep:uint = 0;
      
      protected var _renderers:Array = [];
      
      public function SectionStatusIndicator()
      {
         super();
      }
      
      public function set rendererStep(value:uint) : void
      {
         _rendererStep = value;
         invalidateData();
      }
      
      public function registerRenderer(value:Class) : void
      {
         _progressRenderer = value;
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
         var rendererCount:int = this._renderers.length;
         var j:uint = 0;
         while(j < rendererCount)
         {
            renderer = this._renderers[j];
            renderer.gotoAndStop(j < _value ? 1 : 2);
            j++;
         }
      }
      
      protected function updateRenderers() : void
      {
         var rendererCount:int = _maximum;
         var renderer:MovieClip;
         if(!this._progressRenderer)
         {
            return;
         }
         var j:uint = 0;
         while(j < this._renderers.length)
         {
            renderer = this._renderers[j];
            renderer.gotoAndStop(1);
            removeChild(renderer);
            j++;
         }
         var i:uint = 0;
         var nx:uint;
         var ny:uint;
         while(i < rendererCount)
         {
            if(!this._renderers[i])
            {
               renderer = new this._progressRenderer();
               renderer.gotoAndStop(2);
               this._renderers[i] = renderer;
            }
            else
            {
               renderer = this._renderers[i];
            }
            addChild(renderer);
            nx = this._rendererStep * i;
            ny = 0;
            renderer.x = nx;
            renderer.y = ny;
            i++;
         }
      }
   }
}

