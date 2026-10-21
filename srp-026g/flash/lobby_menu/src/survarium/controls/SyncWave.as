package survarium.controls
{
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.events.Event;
   
   public class SyncWave extends MovieClip
   {
      
      public var size:uint = 1;
      
      protected var mathTargets:Array = [30,-10,30,35,14,40,-7,27,30];
      
      protected var container:MovieClip = new MovieClip();
      
      protected var container2:MovieClip = new MovieClip();
      
      protected var chift:Number = 0;
      
      protected var chift2:Number = 0;
      
      protected var factor2:Number = 0.3141592653589793;
      
      protected var steps:Number = 0;
      
      protected var current_factor:Number = 0;
      
      public function SyncWave()
      {
         super();
         addChild(this.container2);
         this.container2.addChild(this.container);
         this.container.y = 70;
         this.container.x = 6;
         addEventListener(Event.EXIT_FRAME,this.redraw);
      }
      
      public function redraw(param1:Event = null) : void
      {
         chift += this.factor2;
         chift2 = this.chift;
         var _loc2_:Graphics = this.container.graphics;
         _loc2_.clear();
         _loc2_.lineStyle(this.size,6946681,1,false);
         var _loc3_:uint = 40;
         var _loc4_:uint = int(width / _loc3_);
         var _loc5_:Number = Math.PI * 8 / _loc3_;
         _loc2_.moveTo(0,0);
         var _loc6_:Number = 1;
         var _loc7_:Number = Math.PI / _loc3_;
         steps = this.chift * Math.PI;
         var _loc8_:uint = 0;
         var _loc9_:int;
         var _loc10_:Number;
         var _loc11_:Number;
         var _loc12_:Number;
         var _loc13_:Number;
         while(_loc8_ < _loc3_)
         {
            _loc9_ = int(this.steps % 80 / 10);
            _loc10_ = this.mathTargets[_loc9_];
            _loc11_ = this.mathTargets[_loc9_ + 1];
            _loc12_ = this.steps % 10 / 10 * (_loc11_ - _loc10_) + _loc10_;
            _loc13_ = (_loc12_ + _loc6_) / 2;
            _loc6_ = _loc12_;
            chift2 += this.factor2 / 16;
            steps += width / 160;
            _loc2_.lineTo(_loc8_ * _loc4_ - 5 * Math.cos(_loc13_ / 8),(Math.sin(_loc8_ * _loc7_) + 0.4) * 1 * Math.cos(_loc8_ * _loc5_ + this.steps) * _loc13_ * 1);
            _loc8_++;
         }
      }
      
      public function clean() : void
      {
         removeEventListener(Event.EXIT_FRAME,this.redraw);
         var _loc1_:Graphics = this.container.graphics;
         _loc1_.clear();
      }
   }
}

