package survarium.debug
{
   import flash.display.MovieClip;
   
   public class Row extends MovieClip
   {
      
      public static var my_columns:int = 1;
      
      public static var length:uint = 0;
      
      public var rowHeight:int = 20;
      
      protected var elements:Vector.<DebugItem1> = new Vector.<DebugItem1>();
      
      public var _my_width:int = 128;
      
      public function Row()
      {
         super();
      }
      
      public function set my_width(param1:int) : void
      {
         _my_width = param1;
         this.updateElemetnsW();
      }
      
      public function get my_width() : int
      {
         return this._my_width;
      }
      
      public function set length(param1:int) : *
      {
         Row.length = param1;
         var _loc2_:int = 0;
         var _loc3_:DebugItem1;
         while(_loc2_ < param1)
         {
            if(this.elements.length <= _loc2_)
            {
               _loc3_ = new DebugItem1();
               _loc3_.initProp(this.my_width);
               this.elements.push(_loc3_);
               _loc3_.y = _loc2_ * this.rowHeight;
            }
            else
            {
               _loc3_ = this.elements[_loc2_];
            }
            if(!this.contains(_loc3_))
            {
               this.addChild(_loc3_);
            }
            _loc2_++;
         }
         while(_loc2_ < this.elements.length)
         {
            _loc3_ = this.elements[_loc2_];
            this.removeChild(_loc3_);
            _loc2_++;
         }
      }
      
      public function set columns(param1:int) : *
      {
         var _loc2_:int = 0;
         var _loc3_:DebugItem1;
         while(_loc2_ < param1)
         {
            _loc3_ = new DebugItem1();
            _loc3_.initProp(this.my_width);
            addChild(_loc3_);
            this.elements.push(_loc3_);
            _loc3_.y = _loc2_ * this.rowHeight;
            _loc2_++;
         }
      }
      
      public function updateElemetns() : void
      {
         var _loc1_:int = 0;
         var _loc2_:DebugItem1;
         while(_loc1_ < this.elements.length)
         {
            _loc2_ = this.elements[_loc1_];
            _loc2_.y = _loc1_ * this.rowHeight;
            _loc1_++;
         }
      }
      
      public function updateElemetnsW() : void
      {
         var _loc1_:int = 0;
         var _loc2_:DebugItem1;
         while(_loc1_ < this.elements.length)
         {
            _loc2_ = this.elements[_loc1_];
            _loc2_.initProp(this.my_width);
            _loc1_++;
         }
      }
      
      public function addElement(param1:MovieClip) : *
      {
         addChild(param1);
         this.elements.push(param1);
      }
   }
}

