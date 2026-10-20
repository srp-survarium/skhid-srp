package survarium.controls
{
   import flash.display.MovieClip;
   
   public class CaptureBar extends MovieClip
   {
      
      public var pointer:*;
      
      public var body:*;
      
      public var good_mc:*;
      
      public var bad_mc:*;
      
      protected var _percent:int = 50;
      
      protected var good:*;
      
      protected var bad:*;
      
      public function CaptureBar()
      {
         super();
         good = getChildByName("good_mc");
         bad = getChildByName("bad_mc");
      }
      
      public function set percent(_value:int) : void
      {
         if(_value < 0)
         {
            _value = 0;
         }
         if(_value > 100)
         {
            _value = 100;
         }
         this.bad.y = 0;
         this.good.y = 0;
         this.pointer.y = 0;
         _percent = _value;
         this.good.x = 0;
         this.good.width = int(int(this.body.width - 2) * (_value / 100));
         this.bad.width = this.body.width - this.good.width - 2;
         this.bad.x = int(this.good.width);
         this.pointer.x = int(this.bad.x - this.pointer.width / 2);
      }
      
      public function get percent() : int
      {
         return this._percent;
      }
      
      public function swap() : *
      {
         var _temp:* = this.bad;
         bad = this.good;
         good = _temp;
         percent = this._percent;
      }
   }
}

