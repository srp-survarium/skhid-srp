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
      
      public function set my_width(value:int) : void
      {
         _my_width = value;
         this.updateElemetnsW();
      }
      
      public function get my_width() : int
      {
         return this._my_width;
      }
      
      public function set length(value:int) : *
      {
         Row.length = value;
         var i:int = 0;
         var item:DebugItem1;
         while(i < value)
         {
            if(this.elements.length <= i)
            {
               item = new DebugItem1();
               item.initProp(this.my_width);
               this.elements.push(item);
               item.y = i * this.rowHeight;
            }
            else
            {
               item = this.elements[i];
            }
            if(!this.contains(item))
            {
               this.addChild(item);
            }
            i++;
         }
         while(i < this.elements.length)
         {
            item = this.elements[i];
            this.removeChild(item);
            i++;
         }
      }
      
      public function set columns(value:int) : *
      {
         var i:int = 0;
         var item:DebugItem1;
         while(i < value)
         {
            item = new DebugItem1();
            item.initProp(this.my_width);
            addChild(item);
            this.elements.push(item);
            item.y = i * this.rowHeight;
            i++;
         }
      }
      
      public function updateElemetns() : void
      {
         var i:int = 0;
         var item:DebugItem1;
         while(i < this.elements.length)
         {
            item = this.elements[i];
            item.y = i * this.rowHeight;
            i++;
         }
      }
      
      public function updateElemetnsW() : void
      {
         var i:int = 0;
         var item:DebugItem1;
         while(i < this.elements.length)
         {
            item = this.elements[i];
            item.initProp(this.my_width);
            i++;
         }
      }
      
      public function addElement(element:MovieClip) : *
      {
         addChild(element);
         this.elements.push(element);
      }
   }
}

