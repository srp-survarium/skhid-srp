package survarium.item
{
   import scaleform.clik.constants.InvalidationType;
   import survarium.controls.ListItemBase;
   import survarium.data.ItemData;
   
   public class ListItem extends ListItemBase
   {
      
      public var itemRenderer:ItemRendererUnlocked;
      
      public var contentCanvas:*;
      
      public function ListItem()
      {
         super();
         tooltipRendererName = "SrvItemHintRednerer";
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(value:Object) : void
      {
         _data = value;
         invalidateData();
      }
      
      override public function set selected(value:Boolean) : void
      {
         super.selected = value;
         invalidateState();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         mouseChildren = false;
      }
      
      override public function toString() : String
      {
         return "[ListItem " + name + "]";
      }
      
      override protected function updateSlot() : void
      {
         var _itemData:ItemData = this._data as ItemData;
         if(this.contentCanvas)
         {
            this.contentCanvas.visible = false;
         }
         if(this.itemRenderer)
         {
            this.itemRenderer.data = _itemData;
            this.itemRenderer.invalidateData();
            this.itemRenderer.validateNow();
         }
      }
      
      protected function updateSlotState() : void
      {
         trace("ListItem::updateSlotState",this.itemRenderer,_selected,_state);
         if(this.itemRenderer)
         {
            this.itemRenderer.alpha = _selected || _state == "over" ? 1 : 0.7;
            this.itemRenderer.scaleX = this.itemRenderer.scaleY = selected ? 1 : 0.8;
            this.itemRenderer.x = this.itemRenderer.y = selected ? 0 : 20;
            this.itemRenderer.y = _state == "down" ? 1 : 0;
         }
      }
      
      override protected function draw() : void
      {
         visible = _data != null;
         super.draw();
         if(isInvalid(InvalidationType.STATE))
         {
            this.updateSlotState();
         }
         mouseChildren = false;
      }
   }
}

