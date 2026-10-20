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
      
      public static function log(param1:Sprite, param2:String) : void
      {
         if(!_stage)
         {
            return;
         }
         if(!_field[param1])
         {
            _field[param1] = "";
         }
         else
         {
            _field[param1] += "\n" + param2;
         }
         if(!_stage)
         {
            return;
         }
         var _loc3_:Rectangle = param1.getRect(_stage);
         trace("logger::",param1,_loc3_);
         _id.graphics.clear();
         _id.graphics.lineStyle(2,16777215,1);
         _id.graphics.drawRect(_loc3_.x,_loc3_.y,_loc3_.width,_loc3_.height);
         if(_id)
         {
            _id.updateFields();
         }
      }
      
      public static function registerStage(param1:Sprite) : void
      {
         _stage = param1;
      }
      
      override protected function configUI() : void
      {
      }
      
      private function updateFields(param1:Event = null) : void
      {
         var _loc2_:Object;
         var _loc3_:Rectangle;
         var _loc4_:TextField;
         if(!_stage)
         {
            return;
         }
         trace("updateFields",_field);
         for(_loc2_ in _field)
         {
            _loc3_ = _loc2_.getRect(_stage);
            trace("iterate::",textFields[_loc2_],_field[_loc2_]);
            if(!textFields[_loc2_])
            {
               _loc4_ = new TextField();
               textFields[_loc2_] = _loc4_;
               addChild(_loc4_);
            }
            else
            {
               _loc4_ = textFields[_loc2_];
            }
            _loc4_.text = _field[_loc2_];
            _loc4_.y = _loc3_.y - _loc3_.height;
         }
      }
      
      private function resetFields(param1:Event) : void
      {
      }
   }
}

