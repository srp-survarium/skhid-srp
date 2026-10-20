package scaleform.clik.data
{
   public class ListData
   {
      
      public var index:uint = 0;
      
      public var label:String = "Empty";
      
      public var selected:Boolean = false;
      
      public function ListData(index:uint, label:String = "Empty", selected:Boolean = false)
      {
         super();
         this.index = index;
         this.label = label;
         this.selected = selected;
      }
      
      public function toString() : String
      {
         return "[ListData " + this.index + ", " + this.label + ", " + this.selected + "]";
      }
   }
}

