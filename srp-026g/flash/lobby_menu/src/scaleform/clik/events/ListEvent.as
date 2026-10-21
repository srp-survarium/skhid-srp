package scaleform.clik.events
{
   import flash.events.Event;
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public class ListEvent extends Event
   {
      
      public static const ITEM_CLICK:String = "itemClick";
      
      public static const ITEM_PRESS:String = "itemPress";
      
      public static const ITEM_ROLL_OVER:String = "itemRollOver";
      
      public static const ITEM_ROLL_OUT:String = "itemRollOut";
      
      public static const ITEM_DOUBLE_CLICK:String = "itemDoubleClick";
      
      public static const INDEX_CHANGE:String = "listIndexChange";
      
      public var itemRenderer:IListItemRenderer;
      
      public var itemData:Object;
      
      public var index:int = -1;
      
      public var columnIndex:int = -1;
      
      public var rowIndex:int = -1;
      
      public var controllerIdx:uint = 0;
      
      public var buttonIdx:uint = 0;
      
      public var isKeyboard:Boolean = false;
      
      public function ListEvent(type:String, bubbles:Boolean = false, cancelable:Boolean = true, index:int = -1, columnIndex:int = -1, rowIndex:int = -1, itemRenderer:IListItemRenderer = null, itemData:Object = null, controllerIdx:uint = 0, buttonIdx:uint = 0, isKeyboard:Boolean = false)
      {
         super(type,bubbles,cancelable);
         this.itemRenderer = itemRenderer;
         this.itemData = itemData;
         this.index = index;
         this.rowIndex = rowIndex;
         this.columnIndex = columnIndex;
         this.controllerIdx = controllerIdx;
         this.buttonIdx = buttonIdx;
         this.isKeyboard = isKeyboard;
      }
      
      override public function clone() : Event
      {
         return new ListEvent(type,bubbles,cancelable,this.index,this.columnIndex,this.rowIndex,this.itemRenderer,this.itemData,this.controllerIdx,this.buttonIdx,this.isKeyboard);
      }
      
      override public function toString() : String
      {
         return formatToString("ListEvent","type","bubbles","cancelable","index","columnIndex","rowIndex","itemRenderer","itemData","controllerIdx","buttonIdx","isKeyboard");
      }
   }
}

