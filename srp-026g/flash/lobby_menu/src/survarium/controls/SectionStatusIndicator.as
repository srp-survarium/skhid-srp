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
      
      public function set rendererStep(param1:uint) : void
      {
         _rendererStep = param1;
         invalidateData();
      }
      
      public function registerRenderer(param1:Class) : void
      {
         _progressRenderer = param1;
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
         var _loc1_:MovieClip;
         var _loc2_:int = this._renderers.length;
         var _loc3_:uint = 0;
         while(_loc3_ < _loc2_)
         {
            _loc1_ = this._renderers[_loc3_];
            _loc1_.gotoAndStop(_loc3_ < _value ? 1 : 2);
            _loc3_++;
         }
      }
      
      protected function updateRenderers() : void
      {
         var _loc1_:int = _maximum;
         var _loc2_:MovieClip;
         if(!this._progressRenderer)
         {
            return;
         }
         var _loc3_:uint = 0;
         while(_loc3_ < this._renderers.length)
         {
            _loc2_ = this._renderers[_loc3_];
            _loc2_.gotoAndStop(1);
            removeChild(_loc2_);
            _loc3_++;
         }
         var _loc4_:uint = 0;
         var _loc5_:uint;
         var _loc6_:uint;
         while(_loc4_ < _loc1_)
         {
            if(!this._renderers[_loc4_])
            {
               _loc2_ = new this._progressRenderer();
               _loc2_.gotoAndStop(2);
               this._renderers[_loc4_] = _loc2_;
            }
            else
            {
               _loc2_ = this._renderers[_loc4_];
            }
            addChild(_loc2_);
            _loc5_ = this._rendererStep * _loc4_;
            _loc6_ = 0;
            _loc2_.x = _loc5_;
            _loc2_.y = _loc6_;
            _loc4_++;
         }
      }
   }
}

