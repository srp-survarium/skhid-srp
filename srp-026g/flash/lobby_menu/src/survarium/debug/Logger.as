package survarium.debug
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Rectangle;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import scaleform.clik.core.UIComponent;
   
   public class Logger extends UIComponent
   {
      
      protected static var _field:Dictionary = new Dictionary();
      
      protected static var _id:Logger;
      
      protected static var _stage:Sprite;
      
      protected static var textFields:Dictionary = new Dictionary();
      
      public function Logger()
      {
         super();
         _id = this;
      }
      
      public static function log(target:Sprite, message:String) : void
      {
         if(!_stage)
         {
            return;
         }
         if(!_field[target])
         {
            _field[target] = "";
         }
         else
         {
            _field[target] += "\n" + message;
         }
         if(!_stage)
         {
            return;
         }
         var bounds:Rectangle = target.getRect(_stage);
         trace("logger::",target,bounds);
         _id.graphics.clear();
         _id.graphics.lineStyle(2,16777215,1);
         _id.graphics.drawRect(bounds.x,bounds.y,bounds.width,bounds.height);
         if(_id)
         {
            _id.updateFields();
         }
      }
      
      public static function registerStage(stageSprite:Sprite) : void
      {
         _stage = stageSprite;
      }
      
      override protected function configUI() : void
      {
      }
      
      private function updateFields(event:Event = null) : void
      {
         var target:Object;
         var bounds:Rectangle;
         var field:TextField;
         if(!_stage)
         {
            return;
         }
         trace("updateFields",_field);
         for(target in _field)
         {
            bounds = target.getRect(_stage);
            trace("iterate::",textFields[target],_field[target]);
            if(!textFields[target])
            {
               field = new TextField();
               textFields[target] = field;
               addChild(field);
            }
            else
            {
               field = textFields[target];
            }
            field.text = _field[target];
            field.y = bounds.y - bounds.height;
         }
      }
      
      private function resetFields(event:Event) : void
      {
      }
   }
}

