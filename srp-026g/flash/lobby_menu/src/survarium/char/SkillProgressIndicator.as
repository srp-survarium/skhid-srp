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
         var _loc2_:uint = 20;
         var _loc3_:MovieClip;
         trace("updateRenderers",_maximum,this._progressRenderer);
         if(!this._progressRenderer)
         {
            return;
         }
         var _loc4_:int = -90 - _loc1_ / 2 * _loc2_ + _loc2_ / 2;
         var _loc5_:uint = _loc1_;
         while(_loc5_ < this._renderers.length)
         {
            _loc3_ = this._renderers[_loc5_];
            _loc3_.gotoAndStop(1);
            removeChild(_loc3_);
            _loc5_++;
         }
         var _loc6_:uint = 0;
         var _loc7_:Number;
         var _loc8_:Point;
         var _loc9_:Number;
         var _loc10_:Number;
         while(_loc6_ < _loc1_)
         {
            if(!this._renderers[_loc6_])
            {
               _loc3_ = new this._progressRenderer();
               _loc3_.gotoAndStop(2);
               this._renderers[_loc6_] = _loc3_;
               trace("SPI:creteRednderer",_loc3_);
            }
            else
            {
               _loc3_ = this._renderers[_loc6_];
               trace("SPI:updateRenderer",_loc3_);
            }
            addChild(_loc3_);
            _loc7_ = _loc4_ * (Math.PI / 180);
            _loc8_ = new Point(Math.cos(_loc7_),Math.sin(_loc7_));
            _loc9_ = _loc8_.x * this.radius;
            _loc10_ = -this.radius + -_loc8_.y * this.radius;
            _loc3_.x = _loc9_;
            _loc3_.y = _loc10_;
            _loc4_ += _loc2_;
            _loc6_++;
         }
      }
   }
}

